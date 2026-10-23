use crate::app::App;
use crate::app::message::{CaptureMode, Field, Message, Page};
use crate::recorder::AudioSource;
use crate::settings::{MIN_DIM, Preset, Region, ResMode, Settings};

/// Something that stops recording from starting, and the quickest way to fix it.
pub(super) struct Problem {
    pub text: String,
    pub label: &'static str,
    pub action: Message,
}

/// Keeps only what the field can hold: digits, plus one dot for decimals.
pub(super) fn sanitize(field: Field, raw: &str) -> String {
    let mut dot = false;
    raw.chars()
        .filter(|c| match c {
            '0'..='9' => true,
            '.' if field.is_float() && !dot => {
                dot = true;
                true
            }
            _ => false,
        })
        .take(field.max_len())
        .collect()
}

fn parse_u32(t: &str) -> Result<u32, String> {
    let t = t.trim();
    if t.is_empty() {
        return Err("Enter a value".into());
    }
    t.parse::<u32>().map_err(|_| "That number is too large".into())
}

fn parse_f32(t: &str) -> Result<f32, String> {
    let t = t.trim();
    if t.is_empty() {
        return Err("Enter a value".into());
    }
    t.parse::<f32>().map_err(|_| "Enter a number".into())
}

/// Checks one field in isolation by putting it into otherwise valid settings,
/// so the ranges stay defined in one place (`Settings::validate`).
pub(super) fn check_field(f: Field, text: &str) -> Option<String> {
    let run = || -> Result<(), String> {
        let mut s = Settings::default();
        match f {
            Field::Fps => s.fps = parse_u32(text)?,
            Field::Percent => s.res_percent = parse_f32(text)?,
            Field::Width => s.res_w = parse_u32(text)?,
            Field::Height => s.res_h = parse_u32(text)?,
            Field::Bpp => s.bpp = parse_f32(text)?,
            Field::Bitrate => s.bitrate_kbps = parse_u32(text)?,
            Field::QpMin => s.qp_min = parse_u32(text)?.min(255) as u8,
            Field::QpMax => s.qp_max = parse_u32(text)?.min(255) as u8,
            Field::Keyframe => s.keyframe_secs = parse_f32(text)?,
            Field::Threads => s.threads = parse_u32(text)?.min(u16::MAX as u32) as u16,
            Field::AudioKbps => s.audio_kbps = parse_u32(text)?,
            Field::RegX | Field::RegY => {
                parse_u32(text)?;
            }
            Field::RegW | Field::RegH => {
                if parse_u32(text)? < MIN_DIM {
                    return Err(format!("Must be at least {MIN_DIM} px"));
                }
            }
        }
        s.validate()
    };
    run().err()
}

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
        // With no area chosen yet the boxes stay empty instead of showing zeros.
        let r = self.region;
        let show = |v: Option<u32>| v.map(|n| n.to_string()).unwrap_or_default();
        self.txt.insert(Field::RegX, show(r.map(|r| r.x)));
        self.txt.insert(Field::RegY, show(r.map(|r| r.y)));
        self.txt.insert(Field::RegW, show(r.map(|r| r.w)));
        self.txt.insert(Field::RegH, show(r.map(|r| r.h)));
    }

    fn pu32(&self, f: Field, name: &str) -> Result<u32, String> {
        parse_u32(self.txt(f)).map_err(|e| format!("{name}: {}", lower(&e)))
    }

    fn pf32(&self, f: Field, name: &str) -> Result<f32, String> {
        parse_f32(self.txt(f)).map_err(|e| format!("{name}: {}", lower(&e)))
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
        // A hidden, unused field must not block recording.
        if self.source != AudioSource::Off {
            s.audio_kbps = self.pu32(Field::AudioKbps, "AAC bitrate")?;
        }
        s.validate()?;
        Ok(s)
    }

    /// The area as typed, which is the source of truth once an area exists.
    pub(super) fn parse_region(&self) -> Result<Region, String> {
        let x = self.pu32(Field::RegX, "Area X")?;
        let y = self.pu32(Field::RegY, "Area Y")?;
        let w = self.pu32(Field::RegW, "Area width")?;
        let h = self.pu32(Field::RegH, "Area height")?;
        if w < MIN_DIM || h < MIN_DIM {
            return Err(format!("The area must be at least {MIN_DIM} x {MIN_DIM} px"));
        }
        Ok(Region { x, y, w, h })
    }

    pub(super) fn touch(&mut self) {
        self.settings.preset = Preset::Custom;
        if let Ok(s) = self.parse_settings() {
            self.settings = s;
        }
    }

    pub(super) fn commit_region(&mut self) {
        let n = |f| self.txt(f).trim().parse::<u32>().ok();
        if let (Some(x), Some(y), Some(w), Some(h)) =
            (n(Field::RegX), n(Field::RegY), n(Field::RegW), n(Field::RegH))
        {
            self.region = Some(Region { x, y, w, h });
        }
        self.mode = CaptureMode::Region;
    }

    /// Why one field is invalid, for inline display. `None` means fine.
    pub(super) fn field_error(&self, f: Field) -> Option<String> {
        let is_region = matches!(f, Field::RegX | Field::RegY | Field::RegW | Field::RegH);
        // Before any area exists the empty boxes are not an error yet.
        if is_region && self.region.is_none() && self.txt(f).trim().is_empty() {
            return None;
        }
        if let Some(e) = check_field(f, self.txt(f)) {
            return Some(e);
        }
        if matches!(f, Field::QpMin | Field::QpMax) {
            if let (Ok(a), Ok(b)) = (
                parse_u32(self.txt(Field::QpMin)),
                parse_u32(self.txt(Field::QpMax)),
            ) {
                if a > b {
                    return Some("QP min must not be greater than QP max".into());
                }
            }
        }
        None
    }

    fn active_fields(&self) -> Vec<Field> {
        let s = &self.settings;
        let mut v = vec![Field::Fps];
        match s.res_mode {
            ResMode::Percent => v.push(Field::Percent),
            ResMode::Custom => v.extend([Field::Width, Field::Height]),
            _ => {}
        }
        v.push(if s.auto_bitrate { Field::Bpp } else { Field::Bitrate });
        v.extend([Field::QpMin, Field::QpMax, Field::Keyframe, Field::Threads]);
        if self.source != AudioSource::Off {
            v.push(Field::AudioKbps);
        }
        if self.mode == CaptureMode::Region && self.region.is_some() {
            v.extend([Field::RegX, Field::RegY, Field::RegW, Field::RegH]);
        }
        v
    }

    /// The first thing blocking a recording, if any.
    pub(super) fn problem(&self) -> Option<Problem> {
        if self.display.is_none() {
            return Some(Problem {
                text: "No display found. Connect one and refresh.".into(),
                label: "Refresh",
                action: Message::RefreshDisplays,
            });
        }
        if self.mode == CaptureMode::Region && self.region.is_none() {
            return Some(Problem {
                text: "Choose the area to record.".into(),
                label: "Select area",
                action: Message::SelectArea,
            });
        }
        for f in self.active_fields() {
            if let Some(e) = self.field_error(f) {
                return Some(Problem {
                    text: e,
                    label: "Fix",
                    action: Message::Page(Page::Settings(f.tab())),
                });
            }
        }
        // Anything the per-field checks did not catch.
        self.parse_settings().err().map(|e| Problem {
            text: e,
            label: "Open settings",
            action: Message::Page(Page::Settings(crate::app::message::Tab::Video)),
        })
    }
}

fn lower(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_reject_dots_and_letters() {
        assert_eq!(sanitize(Field::Fps, "1a2.5"), "125");
        assert_eq!(sanitize(Field::RegX, "-40"), "40");
    }

    #[test]
    fn decimals_keep_a_single_dot() {
        assert_eq!(sanitize(Field::Bpp, "0.1.5"), "0.15");
        assert_eq!(sanitize(Field::Keyframe, "2.5"), "2.5");
    }

    #[test]
    fn input_length_is_capped() {
        assert_eq!(sanitize(Field::Fps, "123456789"), "123456");
    }

    #[test]
    fn ranges_come_from_settings_validation() {
        assert!(check_field(Field::Fps, "30").is_none());
        assert!(check_field(Field::Fps, "0").is_some());
        assert!(check_field(Field::Fps, "121").is_some());
        assert!(check_field(Field::Fps, "").is_some());
        assert!(check_field(Field::Bpp, "0.1").is_none());
        assert!(check_field(Field::Bpp, "5").is_some());
        assert!(check_field(Field::Bpp, ".").is_some());
        assert!(check_field(Field::QpMax, "52").is_some());
        assert!(check_field(Field::AudioKbps, "320").is_none());
        assert!(check_field(Field::AudioKbps, "321").is_some());
        assert!(check_field(Field::Threads, "999999").is_some());
    }

    #[test]
    fn area_fields() {
        assert!(check_field(Field::RegX, "0").is_none());
        assert!(check_field(Field::RegW, "15").is_some());
        assert!(check_field(Field::RegW, "16").is_none());
        assert!(check_field(Field::RegH, "").is_some());
    }
}
