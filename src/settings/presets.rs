use super::{Effort, Filter, Preset, ResMode, Settings};

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
}
