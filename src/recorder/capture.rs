use super::video::crop_to;
use super::FrameMsg;
use crate::settings::Region;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use xcap::Monitor;

pub struct SendMonitor(pub Monitor);
unsafe impl Send for SendMonitor {}

/// Frames are dropped, not queued, when the encoder falls behind.
pub fn capture_loop(
    sm: SendMonitor,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<FrameMsg>,
    t0: Instant,
    (full_w, full_h): (u32, u32),
    rect: Region,
    fps: u32,
) {
    let SendMonitor(monitor) = sm;
    let step = Duration::from_secs_f64(1.0 / fps as f64);
    let mut next = Instant::now() + step;
    while !stop.load(Ordering::Relaxed) {
        if let Ok(img) = monitor.capture_image() {
            // Ignore frames if the display resolution changed mid-recording.
            if img.width() == full_w && img.height() == full_h {
                let frame = crop_to(img, rect);
                if let Err(TrySendError::Disconnected(_)) = tx.try_send((t0.elapsed(), frame)) {
                    break;
                }
            }
        }
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
            next += step;
        } else {
            next = now + step;
        }
    }
}
