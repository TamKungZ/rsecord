mod audio;
mod capture;
mod video;

use anyhow::{anyhow, Context, Result};
use mp4::{
    AacConfig, AudioObjectType, AvcConfig, ChannelConfig, MediaConfig, Mp4Config, Mp4Writer,
    SampleFreqIndex, TrackConfig, TrackType,
};
use openh264::encoder::Encoder;
use openh264::formats::YUVBuffer;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use xcap::Monitor;

use crate::settings::{Region, Settings, MIN_DIM};
use audio::audio_loop;
use capture::{capture_loop, SendMonitor};
use video::{crop_to, encode_loop, encoder_config, fill_yuv, parse_annexb};

pub(crate) const VIDEO_TIMESCALE: u32 = 90_000;
pub(crate) const AUDIO_RATE: u32 = 48_000;
pub(crate) const VIDEO_TRACK: u32 = 1;
pub(crate) const AUDIO_TRACK: u32 = 2;

pub(crate) type SharedWriter = Arc<Mutex<Mp4Writer<BufWriter<File>>>>;
pub(crate) type FrameMsg = (Duration, xcap::image::RgbaImage);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSource {
    Off,
    Mic,
    System,
}

impl AudioSource {
    pub fn all() -> Vec<AudioSource> {
        vec![AudioSource::Off, AudioSource::Mic, AudioSource::System]
    }
}

impl std::fmt::Display for AudioSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AudioSource::Off => "No audio",
            AudioSource::Mic => "Microphone",
            AudioSource::System => "System audio",
        })
    }
}

pub struct RecordConfig {
    pub source: AudioSource,
    pub display: usize,
    pub region: Option<Region>,
    pub settings: Settings,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    pub index: usize,
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

impl std::fmt::Display for DisplayInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

pub fn list_displays() -> Vec<DisplayInfo> {
    let Ok(monitors) = Monitor::all() else {
        return Vec::new();
    };
    monitors
        .iter()
        .enumerate()
        .map(|(index, m)| {
            let width = m.width().unwrap_or(0);
            let height = m.height().unwrap_or(0);
            let primary = m.is_primary().unwrap_or(false);
            let name = m.name().unwrap_or_default();
            DisplayInfo {
                index,
                label: format!(
                    "{}: {} {}x{}{}",
                    index + 1,
                    name,
                    width,
                    height,
                    if primary { " (primary)" } else { "" }
                ),
                width,
                height,
                primary,
            }
        })
        .collect()
}

fn pick_monitor(display: usize) -> Result<Monitor> {
    let monitors = Monitor::all().map_err(|e| anyhow!("{e}"))?;
    monitors
        .get(display)
        .cloned()
        .or_else(|| {
            monitors
                .iter()
                .find(|m| m.is_primary().unwrap_or(false))
                .cloned()
        })
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| anyhow!("No display found"))
}

pub fn screenshot(display: usize) -> Result<xcap::image::RgbaImage> {
    pick_monitor(display)?
        .capture_image()
        .map_err(|e| anyhow!("{e}"))
}

pub struct Recorder {
    stop: Arc<AtomicBool>,
    capture: JoinHandle<()>,
    encode: JoinHandle<Result<()>>,
    audio: Option<JoinHandle<Result<()>>>,
    writer: SharedWriter,
    out: PathBuf,
}

pub fn output_dir() -> PathBuf {
    let dir = dirs::video_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rsecord");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

impl Recorder {
    pub fn start(cfg: RecordConfig) -> Result<Recorder> {
        let RecordConfig {
            source,
            display,
            region,
            settings,
        } = cfg;
        settings.validate().map_err(|e| anyhow!(e))?;
        let fps = settings.fps;

        let monitor = pick_monitor(display)?;
        let first = monitor.capture_image().map_err(|e| anyhow!("{e}"))?;
        let (full_w, full_h) = (first.width(), first.height());

        let rect = match region {
            Some(r) => r.clamp_to(full_w, full_h).ok_or_else(|| {
                anyhow!(
                    "The selected area does not fit the display ({full_w}x{full_h}) \
                     or is smaller than {MIN_DIM}x{MIN_DIM}"
                )
            })?,
            None => Region {
                x: 0,
                y: 0,
                w: full_w & !1,
                h: full_h & !1,
            },
        };
        let (ow, oh) = settings.output_size(rect.w, rect.h);
        let bps = settings.bitrate_bps(ow, oh);

        let mut enc = Encoder::with_api_config(
            openh264::OpenH264API::from_source(),
            encoder_config(&settings, bps),
        )
        .map_err(|e| anyhow!("Failed to open h264 encoder: {e}"))?;

        let mut yuv = YUVBuffer::new(ow as usize, oh as usize);
        fill_yuv(&crop_to(first, rect), (ow, oh), settings.filter, &mut yuv);
        let (sps, pps, sample) = {
            let bits = enc
                .encode(&yuv)
                .map_err(|e| anyhow!("encode: {e}"))?
                .to_vec();
            let n = parse_annexb(&bits);
            (
                n.sps.ok_or_else(|| anyhow!("SPS not found"))?,
                n.pps.ok_or_else(|| anyhow!("PPS not found"))?,
                n.sample,
            )
        };

        let dir = output_dir();
        let name = chrono::Local::now().format("rsecord-%Y-%m-%d_%H-%M-%S.mp4");
        let out = dir.join(name.to_string());
        let file = BufWriter::new(File::create(&out).context("Failed to create file")?);
        let mut writer = Mp4Writer::write_start(
            file,
            &Mp4Config {
                major_brand: "isom".parse()?,
                minor_version: 512,
                compatible_brands: ["isom", "iso2", "avc1", "mp41"]
                    .iter()
                    .map(|s| s.parse().unwrap())
                    .collect(),
                timescale: 1000,
            },
        )?;
        writer.add_track(&TrackConfig {
            track_type: TrackType::Video,
            timescale: VIDEO_TIMESCALE,
            language: "und".into(),
            media_conf: MediaConfig::AvcConfig(AvcConfig {
                width: ow as u16,
                height: oh as u16,
                seq_param_set: sps,
                pic_param_set: pps,
            }),
        })?;
        if source != AudioSource::Off {
            writer.add_track(&TrackConfig {
                track_type: TrackType::Audio,
                timescale: AUDIO_RATE,
                language: "und".into(),
                media_conf: MediaConfig::AacConfig(AacConfig {
                    bitrate: settings.audio_kbps * 1000,
                    profile: AudioObjectType::AacLowComplexity,
                    freq_index: SampleFreqIndex::Freq48000,
                    chan_conf: ChannelConfig::Stereo,
                }),
            })?;
        }
        let writer: SharedWriter = Arc::new(Mutex::new(writer));

        let stop = Arc::new(AtomicBool::new(false));
        let t0 = Instant::now();

        let audio = if source == AudioSource::Off {
            None
        } else {
            let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
            let (s, wr) = (stop.clone(), writer.clone());
            let kbps = settings.audio_kbps;
            let handle = thread::spawn(move || audio_loop(source, kbps, s, wr, ready_tx));
            match ready_rx.recv() {
                Ok(Ok(())) => Some(handle),
                Ok(Err(e)) => {
                    let _ = handle.join();
                    let _ = std::fs::remove_file(&out);
                    return Err(anyhow!(e));
                }
                Err(_) => {
                    let _ = std::fs::remove_file(&out);
                    return Err(anyhow!("Failed to start audio system"));
                }
            }
        };

        let (tx, rx) = mpsc::sync_channel::<FrameMsg>(3);
        let stop_c = stop.clone();
        let monitor = SendMonitor(monitor);
        let capture = thread::spawn(move || {
            capture_loop(monitor, stop_c, tx, t0, (full_w, full_h), rect, fps);
        });

        let wr = writer.clone();
        let first_sample = (0u64, sample);
        let filter = settings.filter;
        let encode = thread::spawn(move || {
            encode_loop(rx, enc, yuv, (ow, oh), filter, fps, wr, first_sample)
        });

        Ok(Recorder {
            stop,
            capture,
            encode,
            audio,
            writer,
            out,
        })
    }

    pub fn finish(self) -> Result<PathBuf> {
        self.stop.store(true, Ordering::Relaxed);
        self.capture
            .join()
            .map_err(|_| anyhow!("Capture thread panicked"))?;
        self.encode
            .join()
            .map_err(|_| anyhow!("Encode thread panicked"))??;
        if let Some(a) = self.audio {
            a.join().map_err(|_| anyhow!("Audio thread panicked"))??;
        }
        self.writer.lock().unwrap().write_end()?;
        Ok(self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_video_only() {
        let r = Recorder::start(RecordConfig {
            source: AudioSource::Off,
            display: 0,
            region: None,
            settings: Settings::default(),
        })
        .expect("start");
        thread::sleep(Duration::from_secs(3));
        let p = r.finish().expect("finish");
        println!("OUT={}", p.display());
        assert!(std::fs::metadata(&p).unwrap().len() > 1000);
    }
}