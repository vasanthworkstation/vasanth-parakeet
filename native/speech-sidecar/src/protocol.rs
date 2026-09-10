use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum IncomingMessage {
    Initialize { config: Option<SpeechConfig> },
    StartRecording { device_id: Option<String> },
    StopRecording,
    Cancel,
    ListDevices,
    Shutdown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpeechConfig {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_speech_start_threshold")]
    pub speech_start_threshold: f32,
    #[serde(default = "default_speech_continue_threshold")]
    pub speech_continue_threshold: f32,
    #[serde(default = "default_min_speech_ms")]
    pub min_speech_ms: u64,
    #[serde(default = "default_pre_roll_ms")]
    pub pre_roll_ms: u64,
    #[serde(default = "default_post_roll_ms")]
    pub post_roll_ms: u64,
    #[serde(default = "default_endpoint_silence_ms")]
    pub endpoint_silence_ms: u64,
    #[serde(default = "default_max_utterance_ms")]
    pub max_utterance_ms: u64,
}

impl Default for SpeechConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            sample_rate: default_sample_rate(),
            speech_start_threshold: default_speech_start_threshold(),
            speech_continue_threshold: default_speech_continue_threshold(),
            min_speech_ms: default_min_speech_ms(),
            pre_roll_ms: default_pre_roll_ms(),
            post_roll_ms: default_post_roll_ms(),
            endpoint_silence_ms: default_endpoint_silence_ms(),
            max_utterance_ms: default_max_utterance_ms(),
        }
    }
}

fn default_language() -> String { "en".into() }
fn default_sample_rate() -> u32 { 16000 }
fn default_speech_start_threshold() -> f32 { 0.55 }
fn default_speech_continue_threshold() -> f32 { 0.40 }
fn default_min_speech_ms() -> u64 { 250 }
fn default_pre_roll_ms() -> u64 { 250 }
fn default_post_roll_ms() -> u64 { 150 }
fn default_endpoint_silence_ms() -> u64 { 900 }
fn default_max_utterance_ms() -> u64 { 30000 }

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum OutgoingMessage {
    Ready,
    State { state: String },
    Partial { text: String },
    Final { text: String },
    Vad { speech: bool, probability: f32 },
    AudioLevel { value: f32 },
    Devices { devices: Vec<AudioDevice> },
    Error { code: String, message: String, recoverable: bool },
    RecordingStopped,
}

#[derive(Debug, Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

impl OutgoingMessage {
    pub fn send(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            println!("{}", json);
        }
    }

    pub fn error(code: &str, message: &str, recoverable: bool) -> Self {
        Self::Error {
            code: code.to_string(),
            message: message.to_string(),
            recoverable,
        }
    }

    pub fn state(state: &str) -> Self {
        Self::State { state: state.to_string() }
    }
}
