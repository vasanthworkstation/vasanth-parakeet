use rubato::{FftFixedIn, Resampler as RubatoResampler};

use crate::error::SpeechError;

pub struct Resampler {
    resampler: Option<FftFixedIn<f32>>,
    input_rate: u32,
    output_rate: u32,
    chunk_size: usize,
    pending: Vec<f32>,
}

impl Resampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Result<Self, SpeechError> {
        if input_rate == output_rate {
            return Ok(Self {
                resampler: None,
                input_rate,
                output_rate,
                chunk_size: 0,
                pending: Vec::new(),
            });
        }

        let chunk_size = 1024;
        let resampler = FftFixedIn::<f32>::new(
            input_rate as usize,
            output_rate as usize,
            chunk_size,
            2,
            1,
        )
        .map_err(|e| SpeechError::ResampleFailed(e.to_string()))?;

        Ok(Self {
            resampler: Some(resampler),
            input_rate,
            output_rate,
            chunk_size,
            pending: Vec::new(),
        })
    }

    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, SpeechError> {
        let resampler = match &mut self.resampler {
            Some(r) => r,
            None => return Ok(input.to_vec()),
        };

        self.pending.extend_from_slice(input);

        let mut output = Vec::new();

        while self.pending.len() >= self.chunk_size {
            let chunk: Vec<f32> = self.pending.drain(..self.chunk_size).collect();
            let input_frames = vec![chunk];

            let resampled = resampler
                .process(&input_frames, None)
                .map_err(|e| SpeechError::ResampleFailed(e.to_string()))?;

            if let Some(channel) = resampled.into_iter().next() {
                output.extend(channel);
            }
        }

        Ok(output)
    }

    pub fn flush(&mut self) -> Result<Vec<f32>, SpeechError> {
        if self.pending.is_empty() {
            return Ok(Vec::new());
        }

        let resampler = match &mut self.resampler {
            Some(r) => r,
            None => {
                let out = self.pending.clone();
                self.pending.clear();
                return Ok(out);
            }
        };

        while self.pending.len() < self.chunk_size {
            self.pending.push(0.0);
        }

        let chunk: Vec<f32> = self.pending.drain(..self.chunk_size).collect();
        let input_frames = vec![chunk];

        let resampled = resampler
            .process(&input_frames, None)
            .map_err(|e| SpeechError::ResampleFailed(e.to_string()))?;

        self.pending.clear();

        Ok(resampled.into_iter().next().unwrap_or_default())
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    pub fn needs_resample(&self) -> bool {
        self.resampler.is_some()
    }

    pub fn reset(&mut self) {
        self.pending.clear();
        if let Some(r) = &mut self.resampler {
            r.reset();
        }
    }
}
