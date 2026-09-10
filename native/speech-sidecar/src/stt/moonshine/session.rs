pub struct MoonshineSession {
    stable_text: String,
    partial_text: String,
    audio_buffer: Vec<f32>,
    sample_count: usize,
    is_active: bool,
}

impl MoonshineSession {
    pub fn new() -> Self {
        Self {
            stable_text: String::new(),
            partial_text: String::new(),
            audio_buffer: Vec::new(),
            sample_count: 0,
            is_active: false,
        }
    }

    pub fn start(&mut self) {
        self.stable_text.clear();
        self.partial_text.clear();
        self.audio_buffer.clear();
        self.sample_count = 0;
        self.is_active = true;
    }

    pub fn push_audio(&mut self, samples: &[f32]) {
        self.audio_buffer.extend_from_slice(samples);
        self.sample_count += samples.len();
    }

    pub fn get_audio_buffer(&self) -> &[f32] {
        &self.audio_buffer
    }

    pub fn update_partial(&mut self, text: String) {
        self.partial_text = text;
    }

    pub fn get_current_text(&self) -> String {
        if self.partial_text.is_empty() {
            self.stable_text.clone()
        } else {
            format!("{}{}", self.stable_text, self.partial_text)
        }
    }

    pub fn get_partial(&self) -> &str {
        &self.partial_text
    }

    pub fn finalize(&mut self) -> String {
        let final_text = self.get_current_text();
        self.stable_text = final_text.clone();
        self.partial_text.clear();
        self.audio_buffer.clear();
        self.sample_count = 0;
        self.is_active = false;
        final_text
    }

    pub fn reset(&mut self) {
        self.stable_text.clear();
        self.partial_text.clear();
        self.audio_buffer.clear();
        self.sample_count = 0;
        self.is_active = false;
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn audio_duration_ms(&self) -> u64 {
        (self.sample_count as f64 / 16000.0 * 1000.0) as u64
    }
}
