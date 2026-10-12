use super::{AudioSource, SharedWriter, AUDIO_RATE, AUDIO_TRACK};
use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fdk_aac::enc as aac;
use mp4::Mp4Sample;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

const AAC_FRAME: usize = 1024;

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
            if let Ok(desc) = d.description() {
                let n = desc.name().to_lowercase();
                if keys.iter().any(|k| n.contains(k)) {
                    let c = d.default_input_config()?;
                    return Ok((d, c));
                }
            }
        }
        Err(anyhow!("No system audio source found"))
    }
}

pub fn audio_loop(
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

    let mut rs = Resampler {
        ratio: rate as f64 / AUDIO_RATE as f64,
        pos: 0.0,
        buf: Vec::new(),
    };
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
            let info = encoder
                .encode(&chunk, &mut out)
                .map_err(|e| anyhow!("aac: {e}"))?;
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