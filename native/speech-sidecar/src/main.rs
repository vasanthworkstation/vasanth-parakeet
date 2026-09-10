mod audio;
mod error;
mod pipeline;
mod protocol;
mod stt;
mod vad;

use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

use pipeline::coordinator::PipelineCoordinator;
use protocol::{IncomingMessage, OutgoingMessage, SpeechConfig};

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .init();

    log::info!("Speech sidecar starting...");

    let coordinator = Arc::new(Mutex::new(PipelineCoordinator::new()));

    OutgoingMessage::state("initializing").send();

    {
        let mut coord = coordinator.lock().await;
        match coord.initialize(SpeechConfig::default()).await {
            Ok(()) => {
                log::info!("Pipeline initialized successfully");
                OutgoingMessage::Ready.send();
            }
            Err(e) => {
                log::error!("Failed to initialize pipeline: {}", e);
                OutgoingMessage::error(e.error_code(), &e.to_string(), e.is_recoverable()).send();
            }
        }
    }

    let stdin = io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                log::error!("Failed to read stdin: {}", e);
                break;
            }
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let msg: IncomingMessage = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(e) => {
                log::error!("Failed to parse message: {} - input: {}", e, line);
                OutgoingMessage::error(
                    "PROTOCOL_ERROR",
                    &format!("Invalid message: {}", e),
                    true,
                )
                .send();
                continue;
            }
        };

        let mut coord = coordinator.lock().await;

        match msg {
            IncomingMessage::Initialize { config } => {
                let cfg = config.unwrap_or_default();
                match coord.initialize(cfg).await {
                    Ok(()) => {
                        OutgoingMessage::Ready.send();
                    }
                    Err(e) => {
                        OutgoingMessage::error(e.error_code(), &e.to_string(), e.is_recoverable())
                            .send();
                    }
                }
            }

            IncomingMessage::StartRecording { device_id } => {
                OutgoingMessage::state("listening").send();
                match coord.start_recording(device_id).await {
                    Ok(()) => {
                        log::info!("Recording started");
                    }
                    Err(e) => {
                        OutgoingMessage::error(e.error_code(), &e.to_string(), e.is_recoverable())
                            .send();
                    }
                }
            }

            IncomingMessage::StopRecording => {
                OutgoingMessage::state("stopping").send();
                match coord.stop_recording().await {
                    Ok(()) => {
                        OutgoingMessage::RecordingStopped.send();
                        OutgoingMessage::state("ready").send();
                    }
                    Err(e) => {
                        OutgoingMessage::error(e.error_code(), &e.to_string(), e.is_recoverable())
                            .send();
                    }
                }
            }

            IncomingMessage::Cancel => {
                coord.cancel().await;
                OutgoingMessage::state("ready").send();
            }

            IncomingMessage::ListDevices => {
                let devices = coord.list_devices();
                OutgoingMessage::Devices { devices }.send();
            }

            IncomingMessage::Shutdown => {
                log::info!("Shutdown requested");
                coord.shutdown().await;
                OutgoingMessage::state("shutdown").send();
                io::stdout().flush().ok();
                break;
            }
        }

        io::stdout().flush().ok();
    }

    log::info!("Speech sidecar exiting");
}
