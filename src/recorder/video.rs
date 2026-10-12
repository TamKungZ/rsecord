use super::{FrameMsg, SharedWriter, VIDEO_TIMESCALE, VIDEO_TRACK};
use anyhow::{anyhow, Result};
use mp4::Mp4Sample;
use openh264::encoder::Encoder;
use openh264::formats::{RgbaSliceU8, YUVBuffer};
use std::sync::mpsc::Receiver;

pub fn crop<'a>(
    img: &'a xcap::image::RgbaImage,
    w: usize,
    h: usize,
    scratch: &'a mut Vec<u8>,
) -> &'a [u8] {
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

pub struct Nals {
    pub sps: Option<Vec<u8>>,
    pub pps: Option<Vec<u8>>,
    pub sample: Sample,
}

pub struct Sample {
    pub sync: bool,
    pub data: Vec<u8>,
}

pub fn parse_annexb(bits: &[u8]) -> Nals {
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
    let mut n = Nals {
        sps: None,
        pps: None,
        sample: Sample {
            sync: false,
            data: Vec::new(),
        },
    };
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

pub fn encode_loop(
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
        let bits = enc
            .encode(&yuv)
            .map_err(|e| anyhow!("encode: {e}"))?
            .to_vec();
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