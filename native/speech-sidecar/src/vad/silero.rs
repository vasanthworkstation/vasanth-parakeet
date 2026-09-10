use ort::session::Session;
use ort::value::Value;
use std::path::Path;

use crate::error::SpeechError;

const SILERO_SAMPLE_RATE: i64 = 16000;
const SILERO_WINDOW_SIZE: usize = 512; // 32ms at 16kHz

pub struct SileroVad {
    session: Session,
    state: Vec<f32>,   // shape [2, 1, 128] flattened = 256 elements
    sr: Vec<i64>,      // shape [1]
}

impl SileroVad {
    pub fn new(model_path: &Path) -> Result<Self, SpeechError> {
        if !model_path.exists() {
            return Err(SpeechError::VadModelLoadFailed(format!(
                "Model not found: {}",
                model_path.display()
            )));
        }

        let session = Session::builder()
            .map_err(|e| SpeechError::VadModelLoadFailed(e.to_string()))?
            .with_intra_threads(1)
            .map_err(|e| SpeechError::VadModelLoadFailed(e.to_string()))?
            .commit_from_file(model_path)
            .map_err(|e| SpeechError::VadModelLoadFailed(e.to_string()))?;

        let state = vec![0.0f32; 2 * 1 * 128]; // [2, 1, 128] zeros
        let sr = vec![SILERO_SAMPLE_RATE];

        log::info!("Silero VAD loaded from {}", model_path.display());

        Ok(Self { session, state, sr })
    }

    pub fn process_chunk(&mut self, audio: &[f32]) -> Result<f32, SpeechError> {
        if audio.len() != SILERO_WINDOW_SIZE {
            return Err(SpeechError::VadInferenceFailed(format!(
                "Expected {} samples, got {}",
                SILERO_WINDOW_SIZE,
                audio.len()
            )));
        }

        let input_value = Value::from_array(([1usize, SILERO_WINDOW_SIZE], audio.to_vec()))
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;

        let state_value = Value::from_array(([2usize, 1, 128], self.state.clone()))
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;

        let sr_value = Value::from_array(([1usize], self.sr.clone()))
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;

        let outputs = self
            .session
            .run(ort::inputs![input_value, state_value, sr_value])
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;

        // Extract probability — output[0] is shape [1, 1]
        let prob_tensor = outputs[0]
            .try_extract_raw_tensor::<f32>()
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;
        let prob_value = prob_tensor.1.first().copied().unwrap_or(0.0);

        // Extract updated state — output[1] is shape [2, 1, 128]
        let state_tensor = outputs[1]
            .try_extract_raw_tensor::<f32>()
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;
        if state_tensor.1.len() == 256 {
            self.state = state_tensor.1.to_vec();
        }

        Ok(prob_value)
    }

    pub fn reset_state(&mut self) {
        self.state = vec![0.0f32; 2 * 1 * 128];
    }

    pub fn window_size() -> usize {
        SILERO_WINDOW_SIZE
    }

    pub fn sample_rate() -> u32 {
        SILERO_SAMPLE_RATE as u32
    }
}
