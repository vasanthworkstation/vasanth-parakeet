use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpeechError {
    #[error("Microphone unavailable")]
    MicrophoneUnavailable,

    #[error("Microphone disconnected")]
    MicrophoneDisconnected,

    #[error("Audio stream failed: {0}")]
    AudioStreamFailed(String),

    #[error("Audio conversion failed: {0}")]
    AudioConversionFailed(String),

    #[error("Resample failed: {0}")]
    ResampleFailed(String),

    #[error("VAD model load failed: {0}")]
    VadModelLoadFailed(String),

    #[error("VAD inference failed: {0}")]
    VadInferenceFailed(String),

    #[error("Moonshine start failed: {0}")]
    MoonshineStartFailed(String),

    #[error("Moonshine inference failed: {0}")]
    MoonshineInferenceFailed(String),

    #[error("Moonshine exited unexpectedly")]
    MoonshineExited,

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Operation timed out")]
    Timeout,

    #[error("Operation cancelled")]
    Cancelled,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl SpeechError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::MicrophoneUnavailable => "MICROPHONE_UNAVAILABLE",
            Self::MicrophoneDisconnected => "MICROPHONE_DISCONNECTED",
            Self::AudioStreamFailed(_) => "AUDIO_STREAM_FAILED",
            Self::AudioConversionFailed(_) => "AUDIO_CONVERSION_FAILED",
            Self::ResampleFailed(_) => "RESAMPLE_FAILED",
            Self::VadModelLoadFailed(_) => "VAD_MODEL_LOAD_FAILED",
            Self::VadInferenceFailed(_) => "VAD_INFERENCE_FAILED",
            Self::MoonshineStartFailed(_) => "MOONSHINE_START_FAILED",
            Self::MoonshineInferenceFailed(_) => "MOONSHINE_INFERENCE_FAILED",
            Self::MoonshineExited => "MOONSHINE_EXITED",
            Self::ProtocolError(_) => "PROTOCOL_ERROR",
            Self::Timeout => "TIMEOUT",
            Self::Cancelled => "CANCELLED",
            Self::Io(_) => "IO_ERROR",
            Self::Json(_) => "JSON_ERROR",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::MicrophoneDisconnected
                | Self::AudioStreamFailed(_)
                | Self::VadInferenceFailed(_)
                | Self::MoonshineInferenceFailed(_)
                | Self::MoonshineExited
                | Self::Timeout
        )
    }
}
