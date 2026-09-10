#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    Idle,
    Initializing,
    Ready,
    Listening,
    SpeechDetected,
    Streaming,
    EndpointDetected,
    Finalizing,
    Stopping,
    Error,
}

impl PipelineState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Initializing => "initializing",
            Self::Ready => "ready",
            Self::Listening => "listening",
            Self::SpeechDetected => "speech_detected",
            Self::Streaming => "transcribing",
            Self::EndpointDetected => "endpoint_detected",
            Self::Finalizing => "finalizing",
            Self::Stopping => "stopping",
            Self::Error => "error",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Listening | Self::SpeechDetected | Self::Streaming | Self::EndpointDetected | Self::Finalizing
        )
    }

    pub fn can_start_recording(&self) -> bool {
        matches!(self, Self::Ready | Self::Idle)
    }
}
