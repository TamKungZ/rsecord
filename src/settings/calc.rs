use super::{ResMode, Settings, MIN_DIM};

fn range_u32(v: u32, lo: u32, hi: u32, name: &str) -> Result<(), String> {
    if v < lo || v > hi {
        Err(format!("{name} must be between {lo} and {hi}"))
    } else {
        Ok(())
    }
}

fn range_f32(v: f32, lo: f32, hi: f32, name: &str) -> Result<(), String> {
    if !v.is_finite() || v < lo || v > hi {
        Err(format!("{name} must be between {lo} and {hi}"))
    } else {
        Ok(())
    }
}

fn even(v: f64) -> u32 {
    ((v.round() as u32) & !1).max(MIN_DIM)
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        range_u32(self.fps, 1, 120, "FPS")?;
        range_f32(self.res_percent, 10.0, 200.0, "Scale %")?;
        range_u32(self.res_w, MIN_DIM, 7680, "Width")?;
        range_u32(self.res_h, MIN_DIM, 4320, "Height")?;
        range_f32(self.bpp, 0.01, 2.0, "Bits per pixel")?;
        range_u32(self.bitrate_kbps, 100, 100_000, "Bitrate (kbps)")?;
        range_u32(self.qp_min as u32, 0, 51, "QP min")?;
        range_u32(self.qp_max as u32, 0, 51, "QP max")?;
        if self.qp_min > self.qp_max {
            return Err("QP min must not be greater than QP max".into());
        }
        range_f32(self.keyframe_secs, 0.2, 60.0, "Keyframe interval (s)")?;
        range_u32(self.threads as u32, 0, 64, "Threads")?;
        range_u32(self.audio_kbps, 32, 320, "AAC bitrate (kbps)")?;
        Ok(())
    }

    pub fn output_size(&self, sw: u32, sh: u32) -> (u32, u32) {
        let (sw, sh) = (sw.max(MIN_DIM) as f64, sh.max(MIN_DIM) as f64);
        let (w, h) = match self.res_mode {
            ResMode::Source => (sw, sh),
            ResMode::Percent => {
                let k = self.res_percent as f64 / 100.0;
                (sw * k, sh * k)
            }
            ResMode::Custom => {
                if self.keep_aspect {
                    let k = (self.res_w as f64 / sw).min(self.res_h as f64 / sh);
                    (sw * k, sh * k)
                } else {
                    (self.res_w as f64, self.res_h as f64)
                }
            }
            m => {
                // Fixed-height presets never upscale.
                let th = (m.target_height().unwrap_or(sh as u32) as f64).min(sh);
                (sw * th / sh, th)
            }
        };
        (even(w), even(h))
    }

    pub fn bitrate_bps(&self, ow: u32, oh: u32) -> u32 {
        if self.auto_bitrate {
            let bps = ow as f64 * oh as f64 * self.fps as f64 * self.bpp as f64;
            bps.clamp(250_000.0, 100_000_000.0) as u32
        } else {
            self.bitrate_kbps * 1000
        }
    }
}
