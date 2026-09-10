use cpal::traits::HostTrait;
use std::collections::HashSet;
use std::time::{Duration, Instant};

pub struct DeviceWatcher {
    known_devices: HashSet<String>,
    last_check: Instant,
    check_interval: Duration,
}

impl DeviceWatcher {
    pub fn new(check_interval_ms: u64) -> Self {
        let known = Self::current_device_names();
        Self {
            known_devices: known,
            last_check: Instant::now(),
            check_interval: Duration::from_millis(check_interval_ms),
        }
    }

    pub fn check(&mut self) -> DeviceChange {
        let now = Instant::now();
        if now.duration_since(self.last_check) < self.check_interval {
            return DeviceChange::None;
        }
        self.last_check = now;

        let current = Self::current_device_names();

        let added: Vec<String> = current.difference(&self.known_devices).cloned().collect();
        let removed: Vec<String> = self.known_devices.difference(&current).cloned().collect();

        self.known_devices = current;

        if !removed.is_empty() {
            return DeviceChange::Removed(removed);
        }
        if !added.is_empty() {
            return DeviceChange::Added(added);
        }

        DeviceChange::None
    }

    fn current_device_names() -> HashSet<String> {
        let host = cpal::default_host();
        host.input_devices()
            .map(|devs| devs.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default()
    }
}

pub enum DeviceChange {
    None,
    Added(Vec<String>),
    Removed(Vec<String>),
}
