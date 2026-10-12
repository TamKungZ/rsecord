use super::super::AudioSource;
use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::mpsc::Sender;

pub struct Source {
    pub rate: u32,
    inner: Inner,
}

enum Inner {
    Cpal(cpal::Stream),
    #[cfg(target_os = "windows")]
    Wasapi {
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        join: Option<std::thread::JoinHandle<()>>,
    },
}

impl Drop for Source {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if let Inner::Wasapi { stop, join } = &mut self.inner {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(j) = join.take() {
                let _ = j.join();
            }
        }
    }
}

impl Source {
    pub fn open(source: AudioSource, tx: Sender<Vec<[f32; 2]>>) -> Result<Source> {
        match source {
            AudioSource::Mic => open_mic(tx),
            AudioSource::System => open_system(tx),
            AudioSource::Off => Err(anyhow!("audio source is off")),
        }
    }
}

fn open_mic(tx: Sender<Vec<[f32; 2]>>) -> Result<Source> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No microphone found")?;
    let config = device.default_input_config()?;
    let rate = config.sample_rate();
    let stream = build_cpal_stream(&device, config, tx)?;
    Ok(Source {
        rate,
        inner: Inner::Cpal(stream),
    })
}

#[cfg(target_os = "windows")]
fn open_system(tx: Sender<Vec<[f32; 2]>>) -> Result<Source> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use wasapi::{
        initialize_mta, DeviceEnumerator, Direction, SampleType, ShareMode, StreamMode,
        WaveFormat,
    };

    let stop = Arc::new(AtomicBool::new(false));
    let stop_c = stop.clone();
    let (info_tx, info_rx) = std::sync::mpsc::channel::<Result<u32, String>>();

    let join = std::thread::spawn(move || {
        let run = || -> Result<(), String> {
            unsafe {
                initialize_mta().map_err(|e| format!("COM init failed: {e}"))?;
            }

            let enumerator = DeviceEnumerator::new()
                .map_err(|e| format!("device enumerator: {e}"))?;
            let device = enumerator
                .get_default_device(&Direction::Render)
                .map_err(|e| format!("no default render device: {e}"))?;
            let mut client = device
                .get_iaudioclient()
                .map_err(|e| format!("IAudioClient: {e}"))?;
            let mix = client
                .get_mixformat()
                .map_err(|e| format!("mix format: {e}"))?;

            let rate = mix.get_samplespersec();
            let channels = mix.get_nchannels() as usize;

            let want = WaveFormat::new(
                32,
                32,
                &SampleType::Float,
                rate as usize,
                channels,
                None,
            );
            let blockalign = want.get_blockalign() as usize;

            let stream_mode = StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 200_000,
            };

            client
                .initialize_client(&want, &Direction::Capture, &stream_mode)
                .map_err(|e| format!("initialize loopback: {e}"))?;

            let capture = client
                .get_audiocaptureclient()
                .map_err(|e| format!("capture client: {e}"))?;
            let event = client
                .set_get_eventhandle()
                .map_err(|e| format!("event handle: {e}"))?;

            client
                .start_stream()
                .map_err(|e| format!("start stream: {e}"))?;

            let _ = info_tx.send(Ok(rate));

            let mut buf = vec![0u8; blockalign * 4096];
            while !stop_c.load(Ordering::Relaxed) {
                if event.wait_for_event(100).is_err() {
                    continue;
                }
                match capture.read_from_device(blockalign, &mut buf) {
                    Ok((n, _flags)) => {
                        if n == 0 {
                            continue;
                        }
                        let n_bytes = n as usize;
                        let samples: Vec<f32> = buf[..n_bytes]
                            .chunks_exact(4)
                            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                            .collect();

                        let frames: Vec<[f32; 2]> = samples
                            .chunks(channels)
                            .map(|c| {
                                let l = c[0];
                                let r = if channels > 1 { c[1] } else { l };
                                [l, r]
                            })
                            .collect();

                        if tx.send(frames).is_err() {
                            break;
                        }
                    }
                    Err(_) => continue,
                }
            }

            Ok(())
        };

        if let Err(e) = run() {
            let _ = info_tx.send(Err(e));
        }
    });

    match info_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(rate)) => Ok(Source {
            rate,
            inner: Inner::Wasapi {
                stop,
                join: Some(join),
            },
        }),
        Ok(Err(e)) => {
            let _ = join.join();
            Err(anyhow!(e))
        }
        Err(_) => {
            stop.store(true, Ordering::Relaxed);
            let _ = join.join();
            Err(anyhow!("WASAPI loopback init timed out"))
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn open_system(tx: Sender<Vec<[f32; 2]>>) -> Result<Source> {
    let host = cpal::default_host();
    let keys: &[&str] = if cfg!(target_os = "macos") {
        &[
            "blackhole",
            "loopback",
            "soundflower",
            "aggregate",
            "existential audio",
        ]
    } else {
        &["monitor", "loopback"]
    };

    for d in host.input_devices()? {
        if let Ok(desc) = d.description() {
            let n = desc.name().to_lowercase();
            if keys.iter().any(|k| n.contains(k)) {
                let config = d.default_input_config()?;
                let rate = config.sample_rate();
                let stream = build_cpal_stream(&d, config, tx)?;
                return Ok(Source {
                    rate,
                    inner: Inner::Cpal(stream),
                });
            }
        }
    }

    Err(anyhow!(
        "No system audio device found. macOS requires BlackHole, Linux requires a PulseAudio/PipeWire monitor source."
    ))
}

fn build_cpal_stream(
    device: &cpal::Device,
    config: cpal::SupportedStreamConfig,
    tx: Sender<Vec<[f32; 2]>>,
) -> Result<cpal::Stream> {
    use cpal::traits::{DeviceTrait, StreamTrait};

    let ch = config.channels() as usize;
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
    Ok(stream)
}