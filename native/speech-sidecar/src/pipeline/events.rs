use crate::protocol::OutgoingMessage;

pub fn emit_state(state: &str) {
    OutgoingMessage::state(state).send();
}

pub fn emit_partial(text: &str) {
    OutgoingMessage::Partial {
        text: text.to_string(),
    }
    .send();
}

pub fn emit_final(text: &str) {
    OutgoingMessage::Final {
        text: text.to_string(),
    }
    .send();
}

pub fn emit_vad(speech: bool, probability: f32) {
    OutgoingMessage::Vad {
        speech,
        probability,
    }
    .send();
}

pub fn emit_audio_level(value: f32) {
    OutgoingMessage::AudioLevel { value }.send();
}

pub fn emit_error(code: &str, message: &str, recoverable: bool) {
    OutgoingMessage::error(code, message, recoverable).send();
}

pub fn emit_recording_stopped() {
    OutgoingMessage::RecordingStopped.send();
}
