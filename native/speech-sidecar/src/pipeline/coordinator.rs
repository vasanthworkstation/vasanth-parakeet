use std::path::PathBuf;
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender};

use crate::audio::level_meter::LevelMeter;
use crate::audio::normalizer;
use crate::audio::recorder::{AudioRecorder, RecorderConfig};
use crate::audio::resampler::Resampler;
use crate::audio::ring_buffer::RingBuffer;
use crate::error::SpeechError;
use crate::pipeline::events;
use crate::pipeline::state::PipelineState;
use crate::protocol::{AudioDevice, SpeechConfig};
use crate::stt::engine::StreamingSpeechEngine;
use crate::stt::moonshine::engine::MoonshineEngine;
use crate::vad::endpoint_detector::{EndpointDetector, EndpointResult};
use crate::vad::silero::SileroVad;

pub struct PipelineCoordinator {
    state: PipelineState,
    config: SpeechConfig,
    recorder: AudioRecorder,
    audio_rx: Option<Receiver<Vec<f32>>>,
    audio_tx: Option<Sender<Vec<f32>>>,
    resampler: Option<Resampler>,
    ring_buffer: RingBuffer,
    level_meter: LevelMeter,
    vad: Option<SileroVad>,
    endpoint_detector: Option<EndpointDetector>,
    stt_engine: Option<Box<dyn StreamingSpeechEngine>>,
    vad_buffer: Vec<f32>,
    models_dir: PathBuf,
}

impl PipelineCoordinator {
    pub fn new() -> Self {
        let models_dir = Self::resolve_models_dir();

        Self {
            state: PipelineState::Idle,
            config: SpeechConfig::default(),
            recorder: AudioRecorder::new(),
            audio_rx: None,
            audio_tx: None,
            resampler: None,
            ring_buffer: RingBuffer::new(10.0, 16000),
            level_meter: LevelMeter::new(15),
            vad: None,
            endpoint_detector: None,
            stt_engine: None,
            vad_buffer: Vec::new(),
            models_dir,
        }
    }

    fn resolve_models_dir() -> PathBuf {
        // In packaged app: process.resourcesPath / speech / models
        // In dev: native/speech-sidecar/models
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let packaged = exe_dir.join("resources").join("speech").join("models");
        if packaged.exists() {
            return packaged;
        }

        let dev_path = exe_dir.join("models");
        if dev_path.exists() {
            return dev_path;
        }

        PathBuf::from("native/speech-sidecar/models")
    }

    pub async fn initialize(&mut self, config: SpeechConfig) -> Result<(), SpeechError> {
        self.state = PipelineState::Initializing;
        events::emit_state(self.state.as_str());
        self.config = config;

        // Load Silero VAD
        let silero_path = self.models_dir.join("silero").join("silero_vad.onnx");
        match SileroVad::new(&silero_path) {
            Ok(vad) => {
                self.vad = Some(vad);
                log::info!("Silero VAD loaded successfully");
            }
            Err(e) => {
                log::warn!("Silero VAD not available: {}. VAD will be disabled.", e);
            }
        }

        // Initialize endpoint detector
        self.endpoint_detector = Some(EndpointDetector::new(
            self.config.speech_start_threshold,
            self.config.speech_continue_threshold,
            self.config.min_speech_ms,
            self.config.endpoint_silence_ms,
            self.config.max_utterance_ms,
            self.config.sample_rate,
        ));

        // Load Moonshine STT
        let moonshine_dir = self.models_dir.join("moonshine").join("streaming-small");
        let mut engine = MoonshineEngine::new(&moonshine_dir);
        match engine.load() {
            Ok(()) => {
                self.stt_engine = Some(Box::new(engine));
                log::info!("Moonshine STT engine loaded");
            }
            Err(e) => {
                log::warn!("Moonshine STT not available: {}. Transcription will be limited.", e);
            }
        }

        // Create audio channel
        let (tx, rx) = bounded::<Vec<f32>>(1024);
        self.audio_tx = Some(tx);
        self.audio_rx = Some(rx);

        self.state = PipelineState::Ready;
        events::emit_state(self.state.as_str());

        Ok(())
    }

    pub async fn start_recording(&mut self, device_id: Option<String>) -> Result<(), SpeechError> {
        if !self.state.can_start_recording() {
            return Err(SpeechError::ProtocolError(format!(
                "Cannot start recording in state: {:?}",
                self.state
            )));
        }

        let tx = self
            .audio_tx
            .clone()
            .ok_or_else(|| SpeechError::ProtocolError("Audio channel not initialized".into()))?;

        let recorder_config = RecorderConfig {
            device_id,
            preferred_sample_rate: self.config.sample_rate,
        };

        let (actual_rate, _channels) = self.recorder.start(recorder_config, tx)?;

        // Set up resampler if device rate differs from target
        if actual_rate != self.config.sample_rate {
            self.resampler = Some(Resampler::new(actual_rate, self.config.sample_rate)?);
            log::info!("Resampler: {}Hz -> {}Hz", actual_rate, self.config.sample_rate);
        } else {
            self.resampler = None;
        }

        // Reset VAD and STT state
        if let Some(ref mut vad) = self.vad {
            vad.reset_state();
        }
        if let Some(ref mut detector) = self.endpoint_detector {
            detector.reset();
        }
        if let Some(ref mut engine) = self.stt_engine {
            engine.start_session()?;
        }
        self.vad_buffer.clear();
        self.ring_buffer.clear();

        self.state = PipelineState::Listening;
        events::emit_state(self.state.as_str());

        // Start processing loop
        self.run_processing_loop().await;

        Ok(())
    }

    async fn run_processing_loop(&mut self) {
        let rx = match &self.audio_rx {
            Some(rx) => rx.clone(),
            None => return,
        };

        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(samples) => {
                    if !self.state.is_active() {
                        break;
                    }
                    self.process_audio_chunk(&samples);
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    if !self.state.is_active() {
                        break;
                    }
                    continue;
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    log::warn!("Audio channel disconnected");
                    break;
                }
            }
        }
    }

    fn process_audio_chunk(&mut self, raw_samples: &[f32]) {
        // Normalize
        let mut samples = raw_samples.to_vec();
        normalizer::normalize_f32(&mut samples);

        // Resample if needed
        let resampled = if let Some(ref mut resampler) = self.resampler {
            match resampler.process(&samples) {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Resample error: {}", e);
                    return;
                }
            }
        } else {
            samples
        };

        // Audio level
        if let Some(level) = self.level_meter.compute_level(&resampled) {
            events::emit_audio_level(level);
        }

        // Store in ring buffer (for pre-roll)
        self.ring_buffer.push_samples(&resampled);

        // Accumulate for VAD processing (512-sample windows)
        self.vad_buffer.extend_from_slice(&resampled);

        let window_size = SileroVad::window_size();
        while self.vad_buffer.len() >= window_size {
            let chunk: Vec<f32> = self.vad_buffer.drain(..window_size).collect();
            self.process_vad_window(&chunk);
        }
    }

    fn process_vad_window(&mut self, chunk: &[f32]) {
        // Run VAD
        let probability = if let Some(ref mut vad) = self.vad {
            match vad.process_chunk(chunk) {
                Ok(p) => p,
                Err(e) => {
                    log::error!("VAD error: {}", e);
                    return;
                }
            }
        } else {
            // No VAD available - assume speech above RMS threshold
            let rms = normalizer::calculate_rms(chunk);
            if rms > 0.01 { 0.9 } else { 0.1 }
        };

        // Run endpoint detector
        let result = if let Some(ref mut detector) = self.endpoint_detector {
            let is_speech = detector.is_speech();
            events::emit_vad(is_speech, probability);
            detector.process(probability)
        } else {
            return;
        };

        match result {
            EndpointResult::SpeechStarted => {
                self.state = PipelineState::SpeechDetected;
                events::emit_state(self.state.as_str());

                // Push pre-roll audio to STT
                let pre_roll = self.ring_buffer.peek_last_ms(
                    self.config.pre_roll_ms,
                    self.config.sample_rate,
                );
                if let Some(ref mut engine) = self.stt_engine {
                    let _ = engine.push_audio(&pre_roll);
                }

                self.state = PipelineState::Streaming;
                events::emit_state(self.state.as_str());
            }

            EndpointResult::SpeechContinuing | EndpointResult::PossibleEndpoint => {
                // Push audio to STT
                if let Some(ref mut engine) = self.stt_engine {
                    let _ = engine.push_audio(chunk);

                    // Check for partial text
                    if let Some(partial) = engine.partial_text() {
                        if !partial.is_empty() {
                            events::emit_partial(&partial);
                        }
                    }
                }
            }

            EndpointResult::EndpointDetected | EndpointResult::MaxUtteranceReached => {
                self.state = PipelineState::EndpointDetected;
                events::emit_state(self.state.as_str());

                self.state = PipelineState::Finalizing;
                events::emit_state(self.state.as_str());

                // Finalize STT
                if let Some(ref mut engine) = self.stt_engine {
                    match engine.finalize() {
                        Ok(text) => {
                            if !text.is_empty()
                                && !text.starts_with("[transcribing")
                            {
                                events::emit_final(&text);
                            }
                        }
                        Err(e) => {
                            log::error!("STT finalize error: {}", e);
                            events::emit_error(
                                e.error_code(),
                                &e.to_string(),
                                e.is_recoverable(),
                            );
                        }
                    }
                }

                // Reset for next utterance
                if let Some(ref mut vad) = self.vad {
                    vad.reset_state();
                }
                if let Some(ref mut detector) = self.endpoint_detector {
                    detector.reset();
                }
                if let Some(ref mut engine) = self.stt_engine {
                    let _ = engine.start_session();
                }

                self.state = PipelineState::Listening;
                events::emit_state(self.state.as_str());
            }

            EndpointResult::Silence => {}
        }
    }

    pub async fn stop_recording(&mut self) -> Result<(), SpeechError> {
        self.state = PipelineState::Stopping;
        events::emit_state(self.state.as_str());

        // If we have active speech, finalize it
        if let Some(ref mut engine) = self.stt_engine {
            if let Ok(text) = engine.finalize() {
                if !text.is_empty() && !text.starts_with("[transcribing") {
                    events::emit_final(&text);
                }
            }
        }

        self.recorder.stop();

        if let Some(ref mut resampler) = self.resampler {
            resampler.reset();
        }

        self.state = PipelineState::Ready;
        Ok(())
    }

    pub async fn cancel(&mut self) {
        self.state = PipelineState::Stopping;
        self.recorder.stop();

        if let Some(ref mut engine) = self.stt_engine {
            engine.reset();
        }
        if let Some(ref mut vad) = self.vad {
            vad.reset_state();
        }
        if let Some(ref mut detector) = self.endpoint_detector {
            detector.reset();
        }

        self.vad_buffer.clear();
        self.ring_buffer.clear();

        self.state = PipelineState::Ready;
    }

    pub fn list_devices(&self) -> Vec<AudioDevice> {
        AudioRecorder::list_devices()
            .into_iter()
            .map(|(name, is_default)| AudioDevice {
                id: name.clone(),
                name,
                is_default,
            })
            .collect()
    }

    pub async fn shutdown(&mut self) {
        self.recorder.stop();
        if let Some(ref mut engine) = self.stt_engine {
            engine.reset();
        }
        self.state = PipelineState::Idle;
    }

    pub fn get_state(&self) -> PipelineState {
        self.state
    }
}
