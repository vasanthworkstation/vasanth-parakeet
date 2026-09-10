use ndarray::{Array1, Array2, Array3};
use ort::{Session, Value};
use std::path::Path;

use crate::error::SpeechError;

const SILERO_SAMPLE_RATE: i64 = 16000;
const SILERO_WINDOW_SIZE: usize = 512; // 32ms at 16kHz

pub struct SileroVad {
    session: Session,
    state: Array3<f32>,
    sr: Array1<i64>,
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
            .and_then(|b| b.with_intra_threads(1))
            .and_then(|b| b.commit_from_file(model_path))
            .map_err(|e| SpeechError::VadModelLoadFailed(e.to_string()))?;

        let state = Array3::<f32>::zeros((2, 1, 128));
        let sr = Array1::from_vec(vec![SILERO_SAMPLE_RATE]);

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

        let input_array = Array2::from_shape_vec((1, SILERO_WINDOW_SIZE), audio.to_vec())
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;

        let input_value = Value::from_array(input_array)
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;
        let state_value = Value::from_array(self.state.clone())
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;
        let sr_value = Value::from_array(self.sr.clone())
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;

        let outputs = self
            .session
            .run(ort::inputs![input_value, state_value, sr_value])
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;

        let probability = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;
        let prob_value = probability.as_slice().unwrap_or(&[0.0])[0];

        let new_state = outputs[1]
            .try_extract_tensor::<f32>()
            .map_err(|e| SpeechError::VadInferenceFailed(e.to_string()))?;
        if let Ok(state_array) =
            Array3::from_shape_vec((2, 1, 128), new_state.as_slice().unwrap_or(&[]).to_vec())
        {
            self.state = state_array;
        }

        Ok(prob_value)
    }

    pub fn reset_state(&mut self) {
        self.state = Array3::<f32>::zeros((2, 1, 128));
    }

    pub fn window_size() -> usize {
        SILERO_WINDOW_SIZE
    }

    pub fn sample_rate() -> u32 {
        SILERO_SAMPLE_RATE as u32
    }
}
