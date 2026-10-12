mod audio;
mod capture;
mod video;

use anyhow::{anyhow, Context, Result};
use mp4::{
    AacConfig, AudioObjectType, AvcConfig, ChannelConfig, MediaConfig, Mp4Config, Mp4Writer,
    SampleFreqIndex, TrackConfig, TrackType,
};
use openh264::encoder::{
    BitRate, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, RateControlMode, UsageType,
};
use openh264::formats::{RgbaSliceU8, YUVBuffer};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use xcap::Monitor;

use audio::audio_loop;
use capture::{capture_loop, SendMonitor};
use video::{crop, encode_loop, parse_annexb};

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
    pub fn start(source: AudioSource, fps: u32) -> Result<Recorder> {
        let monitors = Monitor::all().map_err(|e| anyhow!("{e}"))?;
        let monitor = monitors
            .iter()
            .position(|m| m.is_primary().unwrap_or(false))
            .map(|i| monitors[i].clone())
            .or_else(|| monitors.into_iter().next())
            .ok_or_else(|| anyhow!("No display found"))?;

        let first = monitor.capture_image().map_err(|e| anyhow!("{e}"))?;
        let (w, h) = ((first.width() & !1) as usize, (first.height() & !1) as usize);
        let (full_w, full_h) = (first.width(), first.height());

        let bps = ((w * h) as u64 * fps as u64 / 10).clamp(1_000_000, 20_000_000) as u32;
        let cfg = EncoderConfig::new()
            .usage_type(UsageType::ScreenContentRealTime)
            .rate_control_mode(RateControlMode::Bitrate)
            .bitrate(BitRate::from_bps(bps))
            .max_frame_rate(FrameRate::from_hz(fps as f32))
            .skip_frames(false)
            .intra_frame_period(IntraFramePeriod::from_num_frames(fps * 2));
        let mut enc = Encoder::with_api_config(openh264::OpenH264API::from_source(), cfg)
            .map_err(|e| anyhow!("Failed to open h264 encoder: {e}"))?;

        let mut yuv = YUVBuffer::new(w, h);
        let mut scratch = Vec::new();
        yuv.read_rgba8(RgbaSliceU8::new(crop(&first, w, h, &mut scratch), (w, h)));
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
                width: w as u16,
                height: h as u16,
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
                    bitrate: 128_000,
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
            let handle = thread::spawn(move || audio_loop(source, s, wr, ready_tx));
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
            capture_loop(monitor, stop_c, tx, t0, full_w, full_h, fps);
        });

        let wr = writer.clone();
        let first_sample = (0u64, sample);
        let encode =
            thread::spawn(move || encode_loop(rx, enc, yuv, (w, h), fps, wr, first_sample));

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
        let r = Recorder::start(AudioSource::Off, 30).expect("start");
        thread::sleep(Duration::from_secs(3));
        let p = r.finish().expect("finish");
        println!("OUT={}", p.display());
        assert!(std::fs::metadata(&p).unwrap().len() > 1000);
    }
}