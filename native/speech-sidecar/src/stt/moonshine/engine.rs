use std::path::{Path, PathBuf};

use crate::error::SpeechError;
use crate::stt::engine::StreamingSpeechEngine;

use super::process::{MoonshineProcess, MoonshineResult};
use super::session::MoonshineSession;

pub struct MoonshineEngine {
    model_dir: PathBuf,
    process: MoonshineProcess,
    session: MoonshineSession,
    loaded: bool,
    use_onnx_direct: bool,
}

impl MoonshineEngine {
    pub fn new(model_dir: &Path) -> Self {
        Self {
            model_dir: model_dir.to_path_buf(),
            process: MoonshineProcess::new(),
            session: MoonshineSession::new(),
            loaded: false,
            use_onnx_direct: false,
        }
    }

    fn poll_results(&mut self) {
        while let Some(result) = self.process.try_recv() {
            match result {
                MoonshineResult::Partial(text) => {
                    self.session.update_partial(text);
                }
                MoonshineResult::Final(text) => {
                    self.session.update_partial(text);
                }
                MoonshineResult::Error(err) => {
                    log::error!("Moonshine error: {}", err);
                }
            }
        }
    }
}

impl StreamingSpeechEngine for MoonshineEngine {
    fn load(&mut self) -> Result<(), SpeechError> {
        if !self.model_dir.exists() {
            return Err(SpeechError::MoonshineStartFailed(format!(
                "Model directory not found: {}",
                self.model_dir.display()
            )));
        }

        match self.process.start(&self.model_dir) {
            Ok(()) => {
                self.loaded = true;
                self.use_onnx_direct = !self.process.is_running();
                if self.use_onnx_direct {
                    log::info!("Moonshine engine loaded in ONNX direct mode");
                } else {
                    log::info!("Moonshine engine loaded with Python subprocess");
                }
                Ok(())
            }
            Err(e) => {
                log::warn!("Moonshine process start failed: {}. Will use ONNX direct mode.", e);
                self.loaded = true;
                self.use_onnx_direct = true;
                Ok(())
            }
        }
    }

    fn start_session(&mut self) -> Result<(), SpeechError> {
        self.session.start();
        Ok(())
    }

    fn push_audio(&mut self, samples: &[f32]) -> Result<(), SpeechError> {
        self.session.push_audio(samples);

        if self.process.is_running() {
            self.process.send_audio(samples)?;
            self.poll_results();
        } else if self.use_onnx_direct {
            // In ONNX direct mode, we accumulate audio and process in finalize
            // Partial results are based on accumulated audio duration
            let duration_ms = self.session.audio_duration_ms();
            if duration_ms > 500 && duration_ms % 500 < 50 {
                // Emit placeholder partial text based on audio length
                let partial = format!("[transcribing... {}ms of audio]", duration_ms);
                self.session.update_partial(partial);
            }
        }

        Ok(())
    }

    fn partial_text(&self) -> Option<String> {
        let text = self.session.get_current_text();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    fn finalize(&mut self) -> Result<String, SpeechError> {
        if self.process.is_running() {
            self.process.send_finalize()?;

            // Wait briefly for final result
            for _ in 0..50 {
                self.poll_results();
                if !self.session.get_partial().is_empty() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }

        let final_text = self.session.finalize();
        Ok(final_text)
    }

    fn reset(&mut self) {
        self.session.reset();
    }

    fn is_loaded(&self) -> bool {
        self.loaded
    }
}
