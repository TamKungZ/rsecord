use super::{FrameMsg, SharedWriter, VIDEO_TIMESCALE, VIDEO_TRACK};
use crate::settings::{Content, Effort, Filter, RateMode, Region, Settings};
use anyhow::{anyhow, Result};
use mp4::Mp4Sample;
use openh264::encoder::{
    BitRate, Complexity, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, QpRange,
    RateControlMode, UsageType,
};
use openh264::formats::{RgbaSliceU8, YUVBuffer};
use std::sync::mpsc::Receiver;
use xcap::image::imageops::{self, FilterType};
use xcap::image::RgbaImage;

/// All encoder tuning lives here. If a newer `openh264` renames a builder
/// method, adjust or delete that one line.
pub fn encoder_config(s: &Settings, bps: u32) -> EncoderConfig {
    let keyframe_frames = ((s.fps as f32 * s.keyframe_secs).round() as u32).max(1);

    let usage = match s.content {
        Content::Screen => UsageType::ScreenContentRealTime,
        Content::Motion => UsageType::CameraVideoRealTime,
    };
    let rate = match s.rate {
        RateMode::Bitrate => RateControlMode::Bitrate,
        RateMode::Quality => RateControlMode::Quality,
    };
    let complexity = match s.effort {
        Effort::Fast => Complexity::Low,
        Effort::Balanced => Complexity::Medium,
        Effort::Best => Complexity::High,
    };

    let mut cfg = EncoderConfig::new()
        .usage_type(usage)
        .rate_control_mode(rate)
        .bitrate(BitRate::from_bps(bps))
        .max_frame_rate(FrameRate::from_hz(s.fps as f32))
        .skip_frames(s.skip_frames)
        .intra_frame_period(IntraFramePeriod::from_num_frames(keyframe_frames))
        .complexity(complexity)
        .qp(QpRange::new(s.qp_min, s.qp_max))
        .adaptive_quantization(s.adaptive_quant)
        .background_detection(s.bg_detect)
        .scene_change_detect(s.scene_detect);
    if s.threads > 0 {
        cfg = cfg.num_threads(s.threads);
    }
    cfg
}

fn filter_type(f: Filter) -> FilterType {
    match f {
        Filter::Nearest => FilterType::Nearest,
        Filter::Bilinear => FilterType::Triangle,
        Filter::Bicubic => FilterType::CatmullRom,
        Filter::Lanczos => FilterType::Lanczos3,
    }
}

pub fn crop_to(img: RgbaImage, r: Region) -> RgbaImage {
    if r.x == 0 && r.y == 0 && r.w == img.width() && r.h == img.height() {
        return img;
    }
    let stride = img.width() as usize * 4;
    let (rw, rh) = (r.w as usize, r.h as usize);
    let mut out = Vec::with_capacity(rw * rh * 4);
    {
        let raw = img.as_raw();
        for row in 0..rh {
            let start = (r.y as usize + row) * stride + r.x as usize * 4;
            out.extend_from_slice(&raw[start..start + rw * 4]);
        }
    }
    RgbaImage::from_raw(r.w, r.h, out).unwrap_or(img)
}

pub fn fill_yuv(img: &RgbaImage, out: (u32, u32), filter: Filter, yuv: &mut YUVBuffer) {
    let dims = (out.0 as usize, out.1 as usize);
    if img.width() == out.0 && img.height() == out.1 {
        yuv.read_rgba8(RgbaSliceU8::new(img.as_raw(), dims));
    } else {
        let scaled = imageops::resize(img, out.0, out.1, filter_type(filter));
        yuv.read_rgba8(RgbaSliceU8::new(scaled.as_raw(), dims));
    }
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
    out: (u32, u32),
    filter: Filter,
    fps: u32,
    wr: SharedWriter,
    first: (u64, Sample),
) -> Result<()> {
    let mut held = Some(first);
    for (pts, img) in rx {
        let mut ticks = (pts.as_micros() as u64 * VIDEO_TIMESCALE as u64) / 1_000_000;
        if let Some((st, _)) = &held {
            ticks = ticks.max(st + 1);
        }
        fill_yuv(&img, out, filter, &mut yuv);
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