//! Recording settings: ranges, presets, validation and persistence.

mod calc;
mod choices;
mod conf;
mod presets;
mod region;
#[cfg(test)]
mod tests;

pub use choices::{AppTheme, Content, Effort, Filter, Preset, RateMode, ResMode};
pub use region::Region;

/// Smallest width/height (pixels) accepted for a capture area or output size.
pub const MIN_DIM: u32 = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub theme: AppTheme,

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
            theme: AppTheme::System,
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
