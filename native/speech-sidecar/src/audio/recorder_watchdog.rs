use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct RecorderWatchdog {
    last_data_time: Arc<AtomicU64>,
    is_stalled: Arc<AtomicBool>,
    stall_threshold_ms: u64,
}

impl RecorderWatchdog {
    pub fn new(stall_threshold_ms: u64) -> Self {
        Self {
            last_data_time: Arc::new(AtomicU64::new(
                Instant::now().elapsed().as_millis() as u64,
            )),
            is_stalled: Arc::new(AtomicBool::new(false)),
            stall_threshold_ms,
        }
    }

    pub fn report_data(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_data_time.store(now, Ordering::Relaxed);
        self.is_stalled.store(false, Ordering::Relaxed);
    }

    pub fn check(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last = self.last_data_time.load(Ordering::Relaxed);
        let elapsed = now.saturating_sub(last);

        if elapsed > self.stall_threshold_ms {
            self.is_stalled.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn is_stalled(&self) -> bool {
        self.is_stalled.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.report_data();
        self.is_stalled.store(false, Ordering::Relaxed);
    }
}
