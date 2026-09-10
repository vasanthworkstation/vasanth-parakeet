pub fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / 32768.0).collect()
}

pub fn u8_to_f32(samples: &[u8]) -> Vec<f32> {
    samples.iter().map(|&s| (s as f32 - 128.0) / 128.0).collect()
}

pub fn f64_to_f32(samples: &[f64]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32).collect()
}

pub fn stereo_to_mono(samples: &[f32]) -> Vec<f32> {
    samples
        .chunks_exact(2)
        .map(|pair| (pair[0] + pair[1]) * 0.5)
        .collect()
}

pub fn multi_channel_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels == 1 {
        return samples.to_vec();
    }
    if channels == 2 {
        return stereo_to_mono(samples);
    }
    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| {
            let sum: f32 = frame.iter().sum();
            sum / ch as f32
        })
        .collect()
}

pub fn pcm_bytes_to_f32_le(bytes: &[u8], bits_per_sample: u16) -> Vec<f32> {
    match bits_per_sample {
        16 => {
            let samples: Vec<i16> = bytes
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect();
            i16_to_f32(&samples)
        }
        8 => u8_to_f32(bytes),
        32 => {
            bytes
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect()
        }
        _ => {
            log::warn!("Unsupported bits per sample: {}", bits_per_sample);
            Vec::new()
        }
    }
}
