pub fn normalize_f32(samples: &mut [f32]) {
    for sample in samples.iter_mut() {
        *sample = sample.clamp(-1.0, 1.0);
    }
}

pub fn apply_gain(samples: &mut [f32], gain: f32) {
    for sample in samples.iter_mut() {
        *sample = (*sample * gain).clamp(-1.0, 1.0);
    }
}

pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

pub fn calculate_peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max)
}
