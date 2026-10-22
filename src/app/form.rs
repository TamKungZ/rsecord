use crate::app::App;
use crate::app::message::{CaptureMode, Field};
use crate::settings::{Preset, Region, ResMode, Settings};

impl App {
    pub(super) fn txt(&self, f: Field) -> &str {
        self.txt.get(&f).map(String::as_str).unwrap_or("")
    }

    pub(super) fn sync_texts(&mut self) {
        let s = &self.settings;
        let t = &mut self.txt;
        t.insert(Field::Fps, s.fps.to_string());
        t.insert(Field::Percent, s.res_percent.to_string());
        t.insert(Field::Width, s.res_w.to_string());
        t.insert(Field::Height, s.res_h.to_string());
        t.insert(Field::Bpp, s.bpp.to_string());
        t.insert(Field::Bitrate, s.bitrate_kbps.to_string());
        t.insert(Field::QpMin, s.qp_min.to_string());
        t.insert(Field::QpMax, s.qp_max.to_string());
        t.insert(Field::Keyframe, s.keyframe_secs.to_string());
        t.insert(Field::Threads, s.threads.to_string());
        t.insert(Field::AudioKbps, s.audio_kbps.to_string());
        self.sync_region_texts();
    }

    pub(super) fn sync_region_texts(&mut self) {
        let r = self.region.unwrap_or(Region {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        });
        self.txt.insert(Field::RegX, r.x.to_string());
        self.txt.insert(Field::RegY, r.y.to_string());
        self.txt.insert(Field::RegW, r.w.to_string());
        self.txt.insert(Field::RegH, r.h.to_string());
    }

    fn pu32(&self, f: Field, name: &str) -> Result<u32, String> {
        self.txt(f)
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("{name}: enter a whole number"))
    }

    fn pf32(&self, f: Field, name: &str) -> Result<f32, String> {
        self.txt(f)
            .trim()
            .parse::<f32>()
            .map_err(|_| format!("{name}: enter a number"))
    }

    pub(super) fn parse_settings(&self) -> Result<Settings, String> {
        let mut s = self.settings.clone();
        s.fps = self.pu32(Field::Fps, "FPS")?;
        if s.res_mode == ResMode::Percent {
            s.res_percent = self.pf32(Field::Percent, "Scale %")?;
        }
        if s.res_mode == ResMode::Custom {
            s.res_w = self.pu32(Field::Width, "Width")?;
            s.res_h = self.pu32(Field::Height, "Height")?;
        }
        if s.auto_bitrate {
            s.bpp = self.pf32(Field::Bpp, "Bits per pixel")?;
        } else {
            s.bitrate_kbps = self.pu32(Field::Bitrate, "Bitrate")?;
        }
        s.qp_min = self.pu32(Field::QpMin, "QP min")?.min(255) as u8;
        s.qp_max = self.pu32(Field::QpMax, "QP max")?.min(255) as u8;
        s.keyframe_secs = self.pf32(Field::Keyframe, "Keyframe interval")?;
        s.threads = self.pu32(Field::Threads, "Threads")?.min(u16::MAX as u32) as u16;
        s.audio_kbps = self.pu32(Field::AudioKbps, "AAC bitrate")?;
        s.validate()?;
        Ok(s)
    }

    pub(super) fn touch(&mut self) {
        self.settings.preset = Preset::Custom;
        match self.parse_settings() {
            Ok(s) => {
                self.settings = s;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(super) fn commit_region(&mut self) {
        let parsed = (
            self.txt(Field::RegX).trim().parse::<u32>().ok(),
            self.txt(Field::RegY).trim().parse::<u32>().ok(),
            self.txt(Field::RegW).trim().parse::<u32>().ok(),
            self.txt(Field::RegH).trim().parse::<u32>().ok(),
        );
        match parsed {
            (Some(x), Some(y), Some(w), Some(h)) => {
                self.region = Some(Region { x, y, w, h });
                self.mode = CaptureMode::Region;
                self.error = None;
            }
            _ => self.error = Some("Area: X, Y, W and H must be whole numbers".into()),
        }
    }
}
