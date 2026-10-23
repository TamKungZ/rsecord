use crate::app::App;
use crate::app::message::{CaptureMode, Message, Status};
use crate::recorder::AudioSource;
use crate::settings::{MIN_DIM, RateMode};
use iced::{window, Task};
use std::path::Path;

impl App {
    pub(super) fn save(&self) {
        self.settings.save(&self.source.to_string());
    }

    pub(super) fn restore_main(&self) -> Task<Message> {
        Task::batch([
            window::minimize(self.main, false),
            window::gain_focus(self.main),
        ])
    }

    pub(super) fn finish_select(&mut self, confirm: bool) -> Task<Message> {
        let (id, region) = match self.selector.as_ref() {
            Some(s) => (s.id, s.region()),
            None => return Task::none(),
        };
        if confirm {
            let Some(r) = region else {
                return Task::none();
            };
            self.region = Some(r);
            self.mode = CaptureMode::Region;
            self.sync_region_texts();
            self.status = Status::info(format!("Area: {}x{} at ({}, {})", r.w, r.h, r.x, r.y));
        } else {
            self.status = Status::info("Area selection cancelled");
        }
        self.selector = None;
        Task::batch([window::close(id), self.restore_main()])
    }

    /// Expected output as two short strings: frame size and rate, then bitrate and size.
    pub(super) fn summary_parts(&self) -> (String, String) {
        let (sw, sh) = match (self.mode, self.region, &self.display) {
            (CaptureMode::Region, Some(r), _) => (r.w, r.h),
            (_, _, Some(d)) => (d.width, d.height),
            _ => (1920, 1080),
        };
        let (sw, sh) = ((sw & !1).max(MIN_DIM), (sh & !1).max(MIN_DIM));
        let s = &self.settings;
        let (ow, oh) = s.output_size(sw, sh);
        let bps = s.bitrate_bps(ow, oh);
        let audio = if self.source == AudioSource::Off {
            0
        } else {
            s.audio_kbps as u64 * 1000
        };
        let mb_min = (bps as u64 + audio) as f64 / 8.0 * 60.0 / 1_048_576.0;
        let tag = match s.rate {
            RateMode::Bitrate => "",
            RateMode::Quality => " (target)",
        };
        (
            format!("{ow} x {oh} at {} fps", s.fps),
            format!("{:.1} Mbps{tag}, about {:.0} MB/min", bps as f64 / 1e6, mb_min),
        )
    }
}

/// Opens the folder holding `file` in the system file manager.
pub(super) fn open_folder(file: &Path) -> Result<(), String> {
    let dir = file.parent().unwrap_or(file);
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(dir)
        .spawn()
        .map(|mut child| {
            // Reap the launcher so it does not linger as a zombie.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        })
        .map_err(|e| format!("Could not open the folder: {e}"))
}
