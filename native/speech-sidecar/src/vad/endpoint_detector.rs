use std::time::Instant;

use super::state::{VadState, VadStateMachine};

pub struct EndpointDetector {
    state_machine: VadStateMachine,
    endpoint_silence_ms: u64,
    max_utterance_ms: u64,
    silence_start: Option<Instant>,
    speech_start: Option<Instant>,
    last_speech_time: Option<Instant>,
}

impl EndpointDetector {
    pub fn new(
        speech_start_threshold: f32,
        speech_continue_threshold: f32,
        min_speech_ms: u64,
        endpoint_silence_ms: u64,
        max_utterance_ms: u64,
        sample_rate: u32,
    ) -> Self {
        let frame_ms = (super::silero::SileroVad::window_size() as f32 / sample_rate as f32) * 1000.0;
        let min_speech_frames = (min_speech_ms as f32 / frame_ms).ceil() as u32;

        Self {
            state_machine: VadStateMachine::new(
                speech_start_threshold,
                speech_continue_threshold,
                min_speech_frames,
            ),
            endpoint_silence_ms,
            max_utterance_ms,
            silence_start: None,
            speech_start: None,
            last_speech_time: None,
        }
    }

    pub fn process(&mut self, probability: f32) -> EndpointResult {
        let prev_state = self.state_machine.state();
        let new_state = self.state_machine.update(probability);
        let now = Instant::now();

        match (prev_state, new_state) {
            (VadState::Silence, VadState::Speech) => {
                self.speech_start = Some(now);
                self.silence_start = None;
                self.last_speech_time = Some(now);
                EndpointResult::SpeechStarted
            }

            (_, VadState::Speech) => {
                self.last_speech_time = Some(now);
                self.silence_start = None;
                EndpointResult::SpeechContinuing
            }

            (VadState::Speech, VadState::PossibleEnd) => {
                self.silence_start = Some(now);
                EndpointResult::PossibleEndpoint
            }

            (VadState::PossibleEnd, VadState::PossibleEnd) => {
                if let Some(silence_start) = self.silence_start {
                    let silence_duration = now.duration_since(silence_start).as_millis() as u64;
                    if silence_duration >= self.endpoint_silence_ms {
                        self.state_machine.force_end();
                        let result = EndpointResult::EndpointDetected;
                        self.reset_timing();
                        return result;
                    }
                }

                if let Some(speech_start) = self.speech_start {
                    let utterance_duration = now.duration_since(speech_start).as_millis() as u64;
                    if utterance_duration >= self.max_utterance_ms {
                        self.state_machine.force_end();
                        let result = EndpointResult::MaxUtteranceReached;
                        self.reset_timing();
                        return result;
                    }
                }

                EndpointResult::PossibleEndpoint
            }

            _ => EndpointResult::Silence,
        }
    }

    fn reset_timing(&mut self) {
        self.silence_start = None;
        self.speech_start = None;
        self.last_speech_time = None;
    }

    pub fn reset(&mut self) {
        self.state_machine.reset();
        self.reset_timing();
    }

    pub fn is_speech(&self) -> bool {
        self.state_machine.state().is_speech()
    }

    pub fn vad_state(&self) -> VadState {
        self.state_machine.state()
    }

    pub fn utterance_duration_ms(&self) -> Option<u64> {
        self.speech_start
            .map(|start| Instant::now().duration_since(start).as_millis() as u64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointResult {
    Silence,
    SpeechStarted,
    SpeechContinuing,
    PossibleEndpoint,
    EndpointDetected,
    MaxUtteranceReached,
}

impl EndpointResult {
    pub fn is_endpoint(&self) -> bool {
        matches!(
            self,
            EndpointResult::EndpointDetected | EndpointResult::MaxUtteranceReached
        )
    }

    pub fn is_speech_active(&self) -> bool {
        matches!(
            self,
            EndpointResult::SpeechStarted
                | EndpointResult::SpeechContinuing
                | EndpointResult::PossibleEndpoint
        )
    }
}
