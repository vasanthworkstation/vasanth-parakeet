use crate::error::SpeechError;

pub trait StreamingSpeechEngine: Send {
    fn load(&mut self) -> Result<(), SpeechError>;
    fn start_session(&mut self) -> Result<(), SpeechError>;
    fn push_audio(&mut self, samples: &[f32]) -> Result<(), SpeechError>;
    fn partial_text(&self) -> Option<String>;
    fn finalize(&mut self) -> Result<String, SpeechError>;
    fn reset(&mut self);
    fn is_loaded(&self) -> bool;
}
