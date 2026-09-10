use std::time::Instant;

use crate::audio::normalizer;

pub struct LevelMeter {
    last_emit: Instant,
    min_interval_ms: u64,
}

impl LevelMeter {
    pub fn new(max_events_per_second: u32) -> Self {
        let min_interval_ms = if max_events_per_second > 0 {
            1000 / max_events_per_second as u64
        } else {
            100
        };
        Self {
            last_emit: Instant::now(),
            min_interval_ms,
        }
    }

    pub fn compute_level(&mut self, samples: &[f32]) -> Option<f32> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_emit).as_millis() as u64;

        if elapsed < self.min_interval_ms {
            return None;
        }

        self.last_emit = now;
        let rms = normalizer::calculate_rms(samples);
        Some(rms.clamp(0.0, 1.0))
    }
}
