use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fdk_aac::enc as aac;
use mp4::{
    AacConfig, AudioObjectType, AvcConfig, ChannelConfig, MediaConfig, Mp4Config, Mp4Sample,
    Mp4Writer, SampleFreqIndex, TrackConfig, TrackType,
};
use openh264::encoder::{
    BitRate, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, RateControlMode, UsageType,
};
use openh264::formats::{RgbaSliceU8, YUVBuffer};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use xcap::Monitor;

const VIDEO_TIMESCALE: u32 = 90_000;
const AUDIO_RATE: u32 = 48_000;
const AAC_FRAME: usize = 1024;
const VIDEO_TRACK: u32 = 1;
const AUDIO_TRACK: u32 = 2;

type SharedWriter = Arc<Mutex<Mp4Writer<BufWriter<File>>>>;
type FrameMsg = (Duration, xcap::image::RgbaImage);

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

struct SendMonitor(Monitor);
unsafe impl Send for SendMonitor {}

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
            let bits = enc.encode(&yuv).map_err(|e| anyhow!("encode: {e}"))?.to_vec();
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

        Ok(Recorder { stop, capture, encode, audio, writer, out })
    }

    pub fn finish(self) -> Result<PathBuf> {
        self.stop.store(true, Ordering::Relaxed);
        self.capture.join().map_err(|_| anyhow!("Capture thread panicked"))?;
        self.encode.join().map_err(|_| anyhow!("Encode thread panicked"))??;
        if let Some(a) = self.audio {
            a.join().map_err(|_| anyhow!("Audio thread panicked"))??;
        }
        self.writer.lock().unwrap().write_end()?;
        Ok(self.out)
    }
}

fn system_device(host: &cpal::Host) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
    #[cfg(target_os = "windows")]
    {
        let d = host.default_output_device().context("No speaker found")?;
        let c = d.default_output_config()?;
        Ok((d, c))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let keys: &[&str] = if cfg!(target_os = "macos") {
            &["blackhole", "loopback", "soundflower", "aggregate"]
        } else {
            &["monitor"]
        };
        for d in host.input_devices()? {
            if let Ok(name) = d.name() {
                let n = name.to_lowercase();
                if keys.iter().any(|k| n.contains(k)) {
                    let c = d.default_input_config()?;
                    return Ok((d, c));
                }
            }
        }
        Err(anyhow!("No system audio source found"))
    }
}

fn capture_loop(
    sm: SendMonitor,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<FrameMsg>,
    t0: Instant,
    full_w: u32,
    full_h: u32,
    fps: u32,
) {
    let SendMonitor(monitor) = sm;
    let step = Duration::from_secs_f64(1.0 / fps as f64);
    let mut next = Instant::now() + step;
    while !stop.load(Ordering::Relaxed) {
        if let Ok(img) = monitor.capture_image() {
            if img.width() == full_w && img.height() == full_h {
                if let Err(TrySendError::Disconnected(_)) = tx.try_send((t0.elapsed(), img)) {
                    break;
                }
            }
        }
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
            next += step;
        } else {
            next = now + step;
        }
    }
}

fn crop<'a>(img: &'a xcap::image::RgbaImage, w: usize, h: usize, scratch: &'a mut Vec<u8>) -> &'a [u8] {
    if img.width() as usize == w && img.height() as usize == h {
        return img.as_raw();
    }
    scratch.clear();
    let stride = img.width() as usize * 4;
    for row in 0..h {
        scratch.extend_from_slice(&img.as_raw()[row * stride..row * stride + w * 4]);
    }
    scratch
}

struct Nals {
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    sample: Sample,
}

struct Sample {
    sync: bool,
    data: Vec<u8>,
}

fn parse_annexb(bits: &[u8]) -> Nals {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= bits.len() {
        if bits[i] == 0 && bits[i + 1] == 0 && bits[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut n = Nals { sps: None, pps: None, sample: Sample { sync: false, data: Vec::new() } };
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map(|x| x - 3).unwrap_or(bits.len());
        while e > s && bits[e - 1] == 0 {
            e -= 1;
        }
        let nal = &bits[s..e];
        if nal.is_empty() {
            continue;
        }
        match nal[0] & 0x1f {
            7 => n.sps = Some(nal.to_vec()),
            8 => n.pps = Some(nal.to_vec()),
            t @ (1 | 5) => {
                n.sample.sync |= t == 5;
                n.sample.data.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                n.sample.data.extend_from_slice(nal);
            }
            _ => {}
        }
    }
    n
}

fn write_video(wr: &SharedWriter, start: u64, dur: u32, s: Sample) -> Result<()> {
    wr.lock().unwrap().write_sample(
        VIDEO_TRACK,
        &Mp4Sample {
            start_time: start,
            duration: dur,
            rendering_offset: 0,
            is_sync: s.sync,
            bytes: s.data.into(),
        },
    )?;
    Ok(())
}

fn encode_loop(
    rx: Receiver<FrameMsg>,
    mut enc: Encoder,
    mut yuv: YUVBuffer,
    (w, h): (usize, usize),
    fps: u32,
    wr: SharedWriter,
    first: (u64, Sample),
) -> Result<()> {
    let mut held = Some(first);
    let mut scratch = Vec::new();
    for (pts, img) in rx {
        let mut ticks = (pts.as_micros() as u64 * VIDEO_TIMESCALE as u64) / 1_000_000;
        if let Some((st, _)) = &held {
            ticks = ticks.max(st + 1);
        }
        yuv.read_rgba8(RgbaSliceU8::new(crop(&img, w, h, &mut scratch), (w, h)));
        let bits = enc.encode(&yuv).map_err(|e| anyhow!("encode: {e}"))?.to_vec();
        let sample = parse_annexb(&bits).sample;
        if sample.data.is_empty() {
            continue;
        }
        if let Some((st, s)) = held.take() {
            write_video(&wr, st, (ticks - st) as u32, s)?;
        }
        held = Some((ticks, sample));
    }
    if let Some((st, s)) = held {
        write_video(&wr, st, VIDEO_TIMESCALE / fps, s)?;
    }
    Ok(())
}

struct Resampler {
    ratio: f64,
    pos: f64,
    buf: Vec<[f32; 2]>,
}

impl Resampler {
    fn pull(&mut self, out: &mut Vec<i16>) {
        while (self.pos as usize) + 1 < self.buf.len() {
            let i = self.pos as usize;
            let f = (self.pos - i as f64) as f32;
            for c in 0..2 {
                let v = self.buf[i][c] * (1.0 - f) + self.buf[i + 1][c] * f;
                out.push((v.clamp(-1.0, 1.0) * 32767.0) as i16);
            }
            self.pos += self.ratio;
        }
        let drop = (self.pos as usize).min(self.buf.len());
        self.buf.drain(..drop);
        self.pos -= drop as f64;
    }
}

fn audio_loop(
    source: AudioSource,
    stop: Arc<AtomicBool>,
    wr: SharedWriter,
    ready: mpsc::Sender<Result<(), String>>,
) -> Result<()> {
    let (tx, rx) = mpsc::channel::<Vec<[f32; 2]>>();
    let setup = || -> Result<(cpal::Stream, u32)> {
        let host = cpal::default_host();
        let (device, config) = match source {
            AudioSource::Mic => {
                let d = host.default_input_device().context("No microphone found")?;
                let c = d.default_input_config()?;
                (d, c)
            }
            _ => system_device(&host)?,
        };
        let ch = config.channels() as usize;
        let rate = config.sample_rate();
        let cfg: cpal::StreamConfig = config.clone().into();
        let err_fn = |e| eprintln!("audio error: {e}");

        macro_rules! build {
            ($t:ty, $conv:expr) => {{
                let tx = tx.clone();
                let cfg = cfg.clone();
                device.build_input_stream(
                    cfg,
                    move |data: &[$t], _| {
                        let frames = data
                            .chunks(ch)
                            .map(|f| {
                                let l: f32 = $conv(f[0]);
                                let r: f32 = if ch > 1 { $conv(f[1]) } else { l };
                                [l, r]
                            })
                            .collect();
                        let _ = tx.send(frames);
                    },
                    err_fn,
                    None,
                )?
            }};
        }
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build!(f32, |s: f32| s),
            cpal::SampleFormat::I16 => build!(i16, |s: i16| s as f32 / 32768.0),
            cpal::SampleFormat::U16 => build!(u16, |s: u16| (s as f32 - 32768.0) / 32768.0),
            f => return Err(anyhow!("Unsupported sample format {f:?}")),
        };
        stream.play()?;
        Ok((stream, rate))
    };

    let (stream, rate) = match setup() {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return Ok(());
        }
    };
    let encoder = match aac::Encoder::new(aac::EncoderParams {
        bit_rate: aac::BitRate::Cbr(128_000),
        sample_rate: AUDIO_RATE,
        transport: aac::Transport::Raw,
        channels: aac::ChannelMode::Stereo,
        audio_object_type: aac::AudioObjectType::Mpeg4LowComplexity,
    }) {
        Ok(e) => e,
        Err(e) => {
            let _ = ready.send(Err(format!("Failed to open aac encoder: {e}")));
            return Ok(());
        }
    };
    let _ = ready.send(Ok(()));

    let mut rs = Resampler { ratio: rate as f64 / AUDIO_RATE as f64, pos: 0.0, buf: Vec::new() };
    let mut pcm: Vec<i16> = Vec::new();
    let mut out = vec![0u8; 8192];
    let mut frames_done: u64 = 0;

    let mut drain = |pcm: &mut Vec<i16>, flush: bool| -> Result<()> {
        if flush && !pcm.is_empty() {
            let pad = AAC_FRAME * 2 - pcm.len() % (AAC_FRAME * 2);
            if pad != AAC_FRAME * 2 {
                pcm.extend(std::iter::repeat(0).take(pad));
            }
        }
        while pcm.len() >= AAC_FRAME * 2 {
            let chunk: Vec<i16> = pcm.drain(..AAC_FRAME * 2).collect();
            let info = encoder.encode(&chunk, &mut out).map_err(|e| anyhow!("aac: {e}"))?;
            if info.output_size > 0 {
                wr.lock().unwrap().write_sample(
                    AUDIO_TRACK,
                    &Mp4Sample {
                        start_time: frames_done,
                        duration: AAC_FRAME as u32,
                        rendering_offset: 0,
                        is_sync: true,
                        bytes: out[..info.output_size].to_vec().into(),
                    },
                )?;
                frames_done += AAC_FRAME as u64;
            }
        }
        Ok(())
    };

    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(frames) => {
                rs.buf.extend(frames);
                rs.pull(&mut pcm);
                drain(&mut pcm, false)?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
    }
    drop(stream);
    loop {
        match rx.try_recv() {
            Ok(frames) => rs.buf.extend(frames),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }
    rs.pull(&mut pcm);
    drain(&mut pcm, true)?;
    Ok(())
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