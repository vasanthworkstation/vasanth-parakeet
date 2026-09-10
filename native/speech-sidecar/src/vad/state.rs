#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadState {
    Silence,
    Speech,
    PossibleEnd,
    EndOfUtterance,
}

impl VadState {
    pub fn is_speech(&self) -> bool {
        matches!(self, VadState::Speech | VadState::PossibleEnd)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            VadState::Silence => "silence",
            VadState::Speech => "speech",
            VadState::PossibleEnd => "possible_end",
            VadState::EndOfUtterance => "end_of_utterance",
        }
    }
}

pub struct VadStateMachine {
    state: VadState,
    speech_start_threshold: f32,
    speech_continue_threshold: f32,
    min_speech_frames: u32,
    speech_frame_count: u32,
}

impl VadStateMachine {
    pub fn new(speech_start_threshold: f32, speech_continue_threshold: f32, min_speech_frames: u32) -> Self {
        Self {
            state: VadState::Silence,
            speech_start_threshold,
            speech_continue_threshold,
            min_speech_frames,
            speech_frame_count: 0,
        }
    }

    pub fn update(&mut self, probability: f32) -> VadState {
        match self.state {
            VadState::Silence => {
                if probability >= self.speech_start_threshold {
                    self.speech_frame_count += 1;
                    if self.speech_frame_count >= self.min_speech_frames {
                        self.state = VadState::Speech;
                    }
                } else {
                    self.speech_frame_count = 0;
                }
            }
            VadState::Speech => {
                if probability < self.speech_continue_threshold {
                    self.state = VadState::PossibleEnd;
                    self.speech_frame_count = 0;
                }
            }
            VadState::PossibleEnd => {
                if probability >= self.speech_continue_threshold {
                    self.state = VadState::Speech;
                }
            }
            VadState::EndOfUtterance => {
                self.state = VadState::Silence;
                self.speech_frame_count = 0;
            }
        }

        self.state
    }

    pub fn force_end(&mut self) {
        self.state = VadState::EndOfUtterance;
        self.speech_frame_count = 0;
    }

    pub fn reset(&mut self) {
        self.state = VadState::Silence;
        self.speech_frame_count = 0;
    }

    pub fn state(&self) -> VadState {
        self.state
    }
}
