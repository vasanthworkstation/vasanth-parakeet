use std::collections::VecDeque;

pub struct RingBuffer {
    buffer: VecDeque<f32>,
    capacity: usize,
    overflow_count: u64,
}

impl RingBuffer {
    pub fn new(capacity_seconds: f32, sample_rate: u32) -> Self {
        let capacity = (capacity_seconds * sample_rate as f32) as usize;
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            overflow_count: 0,
        }
    }

    pub fn push_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            if self.buffer.len() >= self.capacity {
                self.buffer.pop_front();
                self.overflow_count += 1;
            }
            self.buffer.push_back(sample);
        }
    }

    pub fn drain_all(&mut self) -> Vec<f32> {
        self.buffer.drain(..).collect()
    }

    pub fn peek_last_ms(&self, ms: u64, sample_rate: u32) -> Vec<f32> {
        let num_samples = ((ms as f32 / 1000.0) * sample_rate as f32) as usize;
        let start = if self.buffer.len() > num_samples {
            self.buffer.len() - num_samples
        } else {
            0
        };
        self.buffer.iter().skip(start).copied().collect()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn overflow_count(&self) -> u64 {
        self.overflow_count
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}
