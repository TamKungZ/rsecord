mod resampler;
mod source;

use super::{AudioSource, SharedWriter, AUDIO_RATE, AUDIO_TRACK};
use anyhow::{anyhow, Result};
use fdk_aac::enc as aac;
use mp4::Mp4Sample;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use resampler::Resampler;
use source::Source;

const AAC_FRAME: usize = 1024;

pub fn audio_loop(
    source: AudioSource,
    kbps: u32,
    stop: Arc<AtomicBool>,
    wr: SharedWriter,
    ready: mpsc::Sender<Result<(), String>>,
) -> Result<()> {
    let (tx, rx) = mpsc::channel::<Vec<[f32; 2]>>();

    let src = match Source::open(source, tx) {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return Ok(());
        }
    };
    let rate = src.rate;

    let encoder = match aac::Encoder::new(aac::EncoderParams {
        bit_rate: aac::BitRate::Cbr(kbps * 1000),
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

    drop(src);
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