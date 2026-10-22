//! Recording settings: ranges, presets, validation and persistence.

use std::fmt;
use std::path::PathBuf;

/// Smallest width/height (pixels) accepted for a capture area or output size.
pub const MIN_DIM: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Region {
    pub fn clamp_to(self, max_w: u32, max_h: u32) -> Option<Region> {
        if max_w < MIN_DIM || max_h < MIN_DIM || self.w < MIN_DIM || self.h < MIN_DIM {
            return None;
        }
        let x = self.x.min(max_w - MIN_DIM);
        let y = self.y.min(max_h - MIN_DIM);
        let w = self.w.min(max_w - x) & !1;
        let h = self.h.min(max_h - y) & !1;
        if w < MIN_DIM || h < MIN_DIM {
            return None;
        }
        Some(Region { x, y, w, h })
    }
}

macro_rules! choice {
    ($name:ident { $($var:ident => $label:expr),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($var),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(match self { $($name::$var => $label),+ })
            }
        }
    };
}

choice!(Preset {
    Low => "Low (small file)",
    Balanced => "Balanced",
    High => "High quality",
    Ultra => "Ultra (60 fps)",
    Custom => "Custom",
});

choice!(ResMode {
    Source => "Same as source",
    P2160 => "2160p (4K)",
    P1440 => "1440p",
    P1080 => "1080p",
    P720 => "720p",
    P480 => "480p",
    P360 => "360p",
    Percent => "Scale by percent",
    Custom => "Custom size",
});

impl ResMode {
    fn target_height(self) -> Option<u32> {
        match self {
            ResMode::P2160 => Some(2160),
            ResMode::P1440 => Some(1440),
            ResMode::P1080 => Some(1080),
            ResMode::P720 => Some(720),
            ResMode::P480 => Some(480),
            ResMode::P360 => Some(360),
            _ => None,
        }
    }
}

choice!(RateMode {
    Bitrate => "Target bitrate",
    Quality => "Quality (QP range)",
});

choice!(Effort {
    Fast => "Fast (low CPU)",
    Balanced => "Balanced",
    Best => "Best quality (high CPU)",
});

choice!(Filter {
    Nearest => "Nearest (fastest)",
    Bilinear => "Bilinear",
    Bicubic => "Bicubic",
    Lanczos => "Lanczos (sharpest)",
});

choice!(Content {
    Screen => "Screen / UI",
    Motion => "Camera / motion",
});

fn pick<T: Copy + fmt::Display>(all: &[T], s: &str) -> Option<T> {
    all.iter().copied().find(|v| v.to_string() == s)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub preset: Preset,

    pub fps: u32,
    pub res_mode: ResMode,
    pub res_percent: f32,
    pub res_w: u32,
    pub res_h: u32,
    pub keep_aspect: bool,
    pub filter: Filter,

    pub rate: RateMode,
    pub auto_bitrate: bool,
    /// Bits per pixel per frame, used when `auto_bitrate` is on.
    pub bpp: f32,
    pub bitrate_kbps: u32,
    pub qp_min: u8,
    pub qp_max: u8,

    pub effort: Effort,
    pub keyframe_secs: f32,
    pub content: Content,
    pub skip_frames: bool,
    pub adaptive_quant: bool,
    pub bg_detect: bool,
    pub scene_detect: bool,
    /// 0 = let the encoder decide.
    pub threads: u16,

    pub audio_kbps: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            preset: Preset::Balanced,
            fps: 30,
            res_mode: ResMode::Source,
            res_percent: 100.0,
            res_w: 1920,
            res_h: 1080,
            keep_aspect: true,
            filter: Filter::Bilinear,
            rate: RateMode::Bitrate,
            auto_bitrate: true,
            bpp: 0.10,
            bitrate_kbps: 6000,
            qp_min: 0,
            qp_max: 51,
            effort: Effort::Balanced,
            keyframe_secs: 2.0,
            content: Content::Screen,
            skip_frames: false,
            adaptive_quant: true,
            bg_detect: true,
            scene_detect: true,
            threads: 0,
            audio_kbps: 128,
        }
    }
}

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
    pub fn with_preset(p: Preset, current: &Settings) -> Settings {
        let mut s = Settings::default();
        match p {
            Preset::Low => {
                s.fps = 24;
                s.res_mode = ResMode::P720;
                s.bpp = 0.06;
                s.effort = Effort::Fast;
                s.keyframe_secs = 4.0;
                s.audio_kbps = 96;
            }
            Preset::Balanced => {}
            Preset::High => {
                s.bpp = 0.20;
                s.effort = Effort::Best;
                s.filter = Filter::Bicubic;
                s.audio_kbps = 192;
            }
            Preset::Ultra => {
                s.fps = 60;
                s.bpp = 0.18;
                s.effort = Effort::Best;
                s.filter = Filter::Lanczos;
                s.keyframe_secs = 1.0;
                s.audio_kbps = 256;
            }
            Preset::Custom => {
                let mut c = current.clone();
                c.preset = Preset::Custom;
                return c;
            }
        }
        s.preset = p;
        s
    }

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

    pub fn to_conf(&self, audio: &str) -> String {
        let mut o = String::new();
        let mut put = |k: &str, v: String| {
            o.push_str(k);
            o.push('=');
            o.push_str(&v);
            o.push('\n');
        };
        put("audio", audio.to_string());
        put("preset", self.preset.to_string());
        put("fps", self.fps.to_string());
        put("res_mode", self.res_mode.to_string());
        put("res_percent", self.res_percent.to_string());
        put("res_w", self.res_w.to_string());
        put("res_h", self.res_h.to_string());
        put("keep_aspect", self.keep_aspect.to_string());
        put("filter", self.filter.to_string());
        put("rate", self.rate.to_string());
        put("auto_bitrate", self.auto_bitrate.to_string());
        put("bpp", self.bpp.to_string());
        put("bitrate_kbps", self.bitrate_kbps.to_string());
        put("qp_min", self.qp_min.to_string());
        put("qp_max", self.qp_max.to_string());
        put("effort", self.effort.to_string());
        put("keyframe_secs", self.keyframe_secs.to_string());
        put("content", self.content.to_string());
        put("skip_frames", self.skip_frames.to_string());
        put("adaptive_quant", self.adaptive_quant.to_string());
        put("bg_detect", self.bg_detect.to_string());
        put("scene_detect", self.scene_detect.to_string());
        put("threads", self.threads.to_string());
        put("audio_kbps", self.audio_kbps.to_string());
        o
    }

    pub fn apply_conf(&mut self, text: &str) -> Option<String> {
        let mut audio = None;
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "audio" => audio = Some(v.to_string()),
                "preset" => {
                    if let Some(x) = pick(Preset::ALL, v) {
                        self.preset = x;
                    }
                }
                "fps" => {
                    if let Ok(x) = v.parse() {
                        self.fps = x;
                    }
                }
                "res_mode" => {
                    if let Some(x) = pick(ResMode::ALL, v) {
                        self.res_mode = x;
                    }
                }
                "res_percent" => {
                    if let Ok(x) = v.parse() {
                        self.res_percent = x;
                    }
                }
                "res_w" => {
                    if let Ok(x) = v.parse() {
                        self.res_w = x;
                    }
                }
                "res_h" => {
                    if let Ok(x) = v.parse() {
                        self.res_h = x;
                    }
                }
                "keep_aspect" => {
                    if let Ok(x) = v.parse() {
                        self.keep_aspect = x;
                    }
                }
                "filter" => {
                    if let Some(x) = pick(Filter::ALL, v) {
                        self.filter = x;
                    }
                }
                "rate" => {
                    if let Some(x) = pick(RateMode::ALL, v) {
                        self.rate = x;
                    }
                }
                "auto_bitrate" => {
                    if let Ok(x) = v.parse() {
                        self.auto_bitrate = x;
                    }
                }
                "bpp" => {
                    if let Ok(x) = v.parse() {
                        self.bpp = x;
                    }
                }
                "bitrate_kbps" => {
                    if let Ok(x) = v.parse() {
                        self.bitrate_kbps = x;
                    }
                }
                "qp_min" => {
                    if let Ok(x) = v.parse() {
                        self.qp_min = x;
                    }
                }
                "qp_max" => {
                    if let Ok(x) = v.parse() {
                        self.qp_max = x;
                    }
                }
                "effort" => {
                    if let Some(x) = pick(Effort::ALL, v) {
                        self.effort = x;
                    }
                }
                "keyframe_secs" => {
                    if let Ok(x) = v.parse() {
                        self.keyframe_secs = x;
                    }
                }
                "content" => {
                    if let Some(x) = pick(Content::ALL, v) {
                        self.content = x;
                    }
                }
                "skip_frames" => {
                    if let Ok(x) = v.parse() {
                        self.skip_frames = x;
                    }
                }
                "adaptive_quant" => {
                    if let Ok(x) = v.parse() {
                        self.adaptive_quant = x;
                    }
                }
                "bg_detect" => {
                    if let Ok(x) = v.parse() {
                        self.bg_detect = x;
                    }
                }
                "scene_detect" => {
                    if let Ok(x) = v.parse() {
                        self.scene_detect = x;
                    }
                }
                "threads" => {
                    if let Ok(x) = v.parse() {
                        self.threads = x;
                    }
                }
                "audio_kbps" => {
                    if let Ok(x) = v.parse() {
                        self.audio_kbps = x;
                    }
                }
                _ => {}
            }
        }
        audio
    }

    pub fn load() -> (Settings, Option<String>) {
        let mut s = Settings::default();
        let mut audio = None;
        if let Some(text) = conf_path().and_then(|p| std::fs::read_to_string(p).ok()) {
            let mut cand = s.clone();
            let a = cand.apply_conf(&text);
            if cand.validate().is_ok() {
                s = cand;
                audio = a;
            }
        }
        (s, audio)
    }

    pub fn save(&self, audio: &str) {
        if let Some(p) = conf_path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.to_conf(audio));
        }
    }
}

fn conf_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("rsecord").join("settings.conf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid() {
        assert!(Settings::default().validate().is_ok());
        for p in Preset::ALL {
            assert!(Settings::with_preset(*p, &Settings::default()).validate().is_ok());
        }
    }

    #[test]
    fn output_sizes() {
        let mut s = Settings::default();
        assert_eq!(s.output_size(1920, 1080), (1920, 1080));

        s.res_mode = ResMode::P720;
        assert_eq!(s.output_size(1920, 1080), (1280, 720));
        assert_eq!(s.output_size(640, 360), (640, 360));
        assert_eq!(s.output_size(1366, 768), (1280, 720));

        s.res_mode = ResMode::Percent;
        s.res_percent = 50.0;
        assert_eq!(s.output_size(1920, 1080), (960, 540));

        s.res_mode = ResMode::Custom;
        s.res_w = 1000;
        s.res_h = 1000;
        s.keep_aspect = true;
        assert_eq!(s.output_size(1920, 1080), (1000, 562));
        s.keep_aspect = false;
        assert_eq!(s.output_size(1920, 1080), (1000, 1000));
    }

    #[test]
    fn bitrate() {
        let mut s = Settings::default();
        let b = s.bitrate_bps(1920, 1080);
        assert!((6_200_000..6_240_000).contains(&b));
        s.auto_bitrate = false;
        s.bitrate_kbps = 8000;
        assert_eq!(s.bitrate_bps(1920, 1080), 8_000_000);
    }

    #[test]
    fn region_clamp() {
        let r = Region { x: 100, y: 50, w: 801, h: 601 };
        assert_eq!(r.clamp_to(1920, 1080), Some(Region { x: 100, y: 50, w: 800, h: 600 }));
        let r = Region { x: 1900, y: 0, w: 500, h: 500 };
        let c = r.clamp_to(1920, 1080).unwrap();
        assert!(c.x + c.w <= 1920);
        assert_eq!(Region { x: 0, y: 0, w: 8, h: 8 }.clamp_to(1920, 1080), None);
    }

    #[test]
    fn conf_round_trip() {
        let mut a = Settings::with_preset(Preset::Ultra, &Settings::default());
        a.preset = Preset::Custom;
        a.qp_min = 12;
        a.qp_max = 38;
        a.threads = 4;
        a.auto_bitrate = false;
        a.bitrate_kbps = 12345;
        let text = a.to_conf("Microphone");
        let mut b = Settings::default();
        let audio = b.apply_conf(&text);
        assert_eq!(a, b);
        assert_eq!(audio.as_deref(), Some("Microphone"));
    }
}
