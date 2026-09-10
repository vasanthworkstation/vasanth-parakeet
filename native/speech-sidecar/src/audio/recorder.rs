use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use crossbeam_channel::Sender;

use crate::audio::converter;
use crate::error::SpeechError;

pub struct AudioRecorder {
    stream: Option<Stream>,
    device: Option<Device>,
    sample_rate: u32,
    channels: u16,
}

pub struct RecorderConfig {
    pub device_id: Option<String>,
    pub preferred_sample_rate: u32,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            device_id: None,
            preferred_sample_rate: 16000,
        }
    }
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            device: None,
            sample_rate: 0,
            channels: 0,
        }
    }

    pub fn start(
        &mut self,
        config: RecorderConfig,
        sender: Sender<Vec<f32>>,
    ) -> Result<(u32, u16), SpeechError> {
        let host = cpal::default_host();

        let device = if let Some(ref id) = config.device_id {
            host.input_devices()
                .map_err(|e| SpeechError::MicrophoneUnavailable)?
                .find(|d| {
                    d.name()
                        .map(|n| n.contains(id.as_str()))
                        .unwrap_or(false)
                })
                .ok_or(SpeechError::MicrophoneUnavailable)?
        } else {
            host.default_input_device()
                .ok_or(SpeechError::MicrophoneUnavailable)?
        };

        let device_name = device.name().unwrap_or_else(|_| "Unknown".into());
        log::info!("Using input device: {}", device_name);

        let supported_config = device
            .default_input_config()
            .map_err(|e| SpeechError::AudioStreamFailed(e.to_string()))?;

        let sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels();
        let sample_format = supported_config.sample_format();

        log::info!(
            "Device config: {}Hz, {} channels, {:?}",
            sample_rate,
            channels,
            sample_format
        );

        let stream_config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let ch = channels;
        let err_fn = |err: cpal::StreamError| {
            log::error!("Audio stream error: {}", err);
        };

        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx = sender.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mono = converter::multi_channel_to_mono(data, ch);
                        let _ = tx.try_send(mono);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let tx = sender.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let float_data = converter::i16_to_f32(data);
                        let mono = converter::multi_channel_to_mono(&float_data, ch);
                        let _ = tx.try_send(mono);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U8 => {
                let tx = sender.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        let float_data = converter::u8_to_f32(data);
                        let mono = converter::multi_channel_to_mono(&float_data, ch);
                        let _ = tx.try_send(mono);
                    },
                    err_fn,
                    None,
                )
            }
            _ => {
                return Err(SpeechError::AudioStreamFailed(format!(
                    "Unsupported sample format: {:?}",
                    sample_format
                )));
            }
        }
        .map_err(|e| SpeechError::AudioStreamFailed(e.to_string()))?;

        stream
            .play()
            .map_err(|e| SpeechError::AudioStreamFailed(e.to_string()))?;

        self.stream = Some(stream);
        self.device = Some(device);
        self.sample_rate = sample_rate;
        self.channels = channels;

        Ok((sample_rate, channels))
    }

    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        log::info!("Audio recorder stopped");
    }

    pub fn is_active(&self) -> bool {
        self.stream.is_some()
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn list_devices() -> Vec<(String, bool)> {
        let host = cpal::default_host();
        let default_name = host
            .default_input_device()
            .and_then(|d| d.name().ok())
            .unwrap_or_default();

        host.input_devices()
            .map(|devices| {
                devices
                    .filter_map(|d| {
                        d.name().ok().map(|name| {
                            let is_default = name == default_name;
                            (name, is_default)
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        self.stop();
    }
}
