use super::FrameMsg;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use xcap::Monitor;

pub struct SendMonitor(pub Monitor);
unsafe impl Send for SendMonitor {}

pub fn capture_loop(
    sm: SendMonitor,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<FrameMsg>,
    t0: Instant,
    full_w: u32,
    full_h: u32,
    fps: u32,
) {
    let SendMonitor(monitor) = sm;
    let step = Duration::from_secs_f64(1.0 / fps as f64);
    let mut next = Instant::now() + step;
    while !stop.load(Ordering::Relaxed) {
        if let Ok(img) = monitor.capture_image() {
            if img.width() == full_w && img.height() == full_h {
                if let Err(TrySendError::Disconnected(_)) = tx.try_send((t0.elapsed(), img)) {
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