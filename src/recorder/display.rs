use anyhow::{anyhow, Result};
use xcap::Monitor;

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    pub index: usize,
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

impl std::fmt::Display for DisplayInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

pub fn list_displays() -> Vec<DisplayInfo> {
    let Ok(monitors) = Monitor::all() else {
        return Vec::new();
    };
    monitors
        .iter()
        .enumerate()
        .map(|(index, m)| {
            let width = m.width().unwrap_or(0);
            let height = m.height().unwrap_or(0);
            let primary = m.is_primary().unwrap_or(false);
            let name = m.name().unwrap_or_default();
            DisplayInfo {
                index,
                label: format!(
                    "{}: {} {}x{}{}",
                    index + 1,
                    name,
                    width,
                    height,
                    if primary { " (primary)" } else { "" }
                ),
                width,
                height,
                primary,
            }
        })
        .collect()
}

pub(super) fn pick_monitor(display: usize) -> Result<Monitor> {
    let monitors = Monitor::all().map_err(|e| anyhow!("{e}"))?;
    monitors
        .get(display)
        .cloned()
        .or_else(|| {
            monitors
                .iter()
                .find(|m| m.is_primary().unwrap_or(false))
                .cloned()
        })
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| anyhow!("No display found"))
}

pub fn screenshot(display: usize) -> Result<xcap::image::RgbaImage> {
    pick_monitor(display)?
        .capture_image()
        .map_err(|e| anyhow!("{e}"))
}
