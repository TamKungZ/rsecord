use super::choices::pick;
use super::{AppTheme, Content, Effort, Filter, Preset, RateMode, ResMode, Settings};
use std::path::PathBuf;

impl Settings {
    pub fn to_conf(&self, audio: &str) -> String {
        let mut o = String::new();
        let mut put = |k: &str, v: String| {
            o.push_str(k);
            o.push('=');
            o.push_str(&v);
            o.push('\n');
        };
        put("audio", audio.to_string());
        put("theme", self.theme.to_string());
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
                "theme" => {
                    if let Some(x) = pick(AppTheme::ALL, v) {
                        self.theme = x;
                    }
                }
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
