use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;

use crate::error::SpeechError;

pub struct MoonshineProcess {
    child: Option<Child>,
    result_rx: Option<mpsc::Receiver<MoonshineResult>>,
}

#[derive(Debug)]
pub enum MoonshineResult {
    Partial(String),
    Final(String),
    Error(String),
}

impl MoonshineProcess {
    pub fn new() -> Self {
        Self {
            child: None,
            result_rx: None,
        }
    }

    pub fn start(&mut self, model_dir: &Path) -> Result<(), SpeechError> {
        let moonshine_script = model_dir.join("moonshine_server.py");

        if !moonshine_script.exists() {
            log::warn!(
                "Moonshine server script not found at {}. Using ONNX runtime fallback.",
                moonshine_script.display()
            );
            return Ok(());
        }

        let mut child = Command::new("python")
            .arg(&moonshine_script)
            .arg("--model-dir")
            .arg(model_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SpeechError::MoonshineStartFailed(e.to_string()))?;

        let stdout = child.stdout.take().ok_or_else(|| {
            SpeechError::MoonshineStartFailed("Failed to capture stdout".into())
        })?;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&text) {
                            let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            let text_val = msg
                                .get("text")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();
                            match msg_type {
                                "partial" => {
                                    let _ = tx.send(MoonshineResult::Partial(text_val));
                                }
                                "final" => {
                                    let _ = tx.send(MoonshineResult::Final(text_val));
                                }
                                "error" => {
                                    let _ = tx.send(MoonshineResult::Error(text_val));
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(MoonshineResult::Error(e.to_string()));
                        break;
                    }
                }
            }
        });

        self.child = Some(child);
        self.result_rx = Some(rx);

        Ok(())
    }

    pub fn send_audio(&mut self, samples: &[f32]) -> Result<(), SpeechError> {
        if let Some(ref mut child) = self.child {
            if let Some(ref mut stdin) = child.stdin {
                let msg = serde_json::json!({
                    "type": "audio",
                    "samples": samples.len(),
                    "data": samples
                });
                let line = serde_json::to_string(&msg)
                    .map_err(|e| SpeechError::MoonshineInferenceFailed(e.to_string()))?;
                writeln!(stdin, "{}", line)
                    .map_err(|e| SpeechError::MoonshineInferenceFailed(e.to_string()))?;
                stdin
                    .flush()
                    .map_err(|e| SpeechError::MoonshineInferenceFailed(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn send_finalize(&mut self) -> Result<(), SpeechError> {
        if let Some(ref mut child) = self.child {
            if let Some(ref mut stdin) = child.stdin {
                let msg = serde_json::json!({"type": "finalize"});
                let line = serde_json::to_string(&msg)
                    .map_err(|e| SpeechError::MoonshineInferenceFailed(e.to_string()))?;
                writeln!(stdin, "{}", line)
                    .map_err(|e| SpeechError::MoonshineInferenceFailed(e.to_string()))?;
                stdin
                    .flush()
                    .map_err(|e| SpeechError::MoonshineInferenceFailed(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn try_recv(&self) -> Option<MoonshineResult> {
        self.result_rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
    }

    pub fn stop(&mut self) {
        if let Some(ref mut child) = self.child {
            if let Some(ref mut stdin) = child.stdin {
                let msg = serde_json::json!({"type": "shutdown"});
                if let Ok(line) = serde_json::to_string(&msg) {
                    let _ = writeln!(stdin, "{}", line);
                    let _ = stdin.flush();
                }
            }
            let _ = child.wait();
        }
        self.child = None;
        self.result_rx = None;
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

impl Drop for MoonshineProcess {
    fn drop(&mut self) {
        self.stop();
    }
}
