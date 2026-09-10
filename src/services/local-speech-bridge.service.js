const { EventEmitter } = require('events');
const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const readline = require('readline');
const logger = require('../core/logger').createServiceLogger('SPEECH-BRIDGE');

class LocalSpeechBridge extends EventEmitter {
  constructor(options = {}) {
    super();
    this.process = null;
    this.rl = null;
    this.state = 'idle';
    this.isReady = false;
    this.restartCount = 0;
    this.maxRestarts = 3;
    this.restartDelays = [500, 1000, 2000];
    this.shutdownRequested = false;

    this.config = {
      language: options.language || 'en',
      sample_rate: options.sampleRate || 16000,
      speech_start_threshold: options.speechStartThreshold || 0.55,
      speech_continue_threshold: options.speechContinueThreshold || 0.40,
      min_speech_ms: options.minSpeechMs || 250,
      pre_roll_ms: options.preRollMs || 250,
      post_roll_ms: options.postRollMs || 150,
      endpoint_silence_ms: options.endpointSilenceMs || 900,
      max_utterance_ms: options.maxUtteranceMs || 30000,
    };
  }

  _resolveSidecarPath() {
    // Check packaged app location first
    if (process.resourcesPath) {
      const packaged = path.join(process.resourcesPath, 'speech', 'speech-sidecar.exe');
      if (fs.existsSync(packaged)) {
        return packaged;
      }
    }

    // Development paths
    const devPaths = [
      path.join(__dirname, '..', '..', 'native', 'speech-sidecar', 'target', 'release', 'speech-sidecar.exe'),
      path.join(__dirname, '..', '..', 'native', 'speech-sidecar', 'target', 'debug', 'speech-sidecar.exe'),
      path.join(process.cwd(), 'native', 'speech-sidecar', 'target', 'release', 'speech-sidecar.exe'),
      path.join(process.cwd(), 'native', 'speech-sidecar', 'target', 'debug', 'speech-sidecar.exe'),
    ];

    for (const p of devPaths) {
      if (fs.existsSync(p)) {
        return p;
      }
    }

    return null;
  }

  async startSidecar() {
    if (this.process) {
      logger.warn('Sidecar already running');
      return;
    }

    const sidecarPath = this._resolveSidecarPath();
    if (!sidecarPath) {
      const msg = 'Speech sidecar executable not found. Please build the native sidecar first (npm run speech:build).';
      logger.error(msg);
      this.emit('error', { code: 'SIDECAR_NOT_FOUND', message: msg, recoverable: false });
      return;
    }

    logger.info('Starting speech sidecar', { path: sidecarPath });

    this.shutdownRequested = false;

    this.process = spawn(sidecarPath, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
    });

    this.process.on('error', (err) => {
      logger.error('Sidecar process error', { error: err.message });
      this.emit('error', { code: 'SIDECAR_ERROR', message: err.message, recoverable: true });
      this._handleProcessExit();
    });

    this.process.on('exit', (code, signal) => {
      logger.info('Sidecar process exited', { code, signal });
      this.process = null;
      this.isReady = false;
      this.rl = null;

      if (!this.shutdownRequested) {
        this._handleProcessExit();
      }
    });

    // Capture stderr for logging
    this.process.stderr.on('data', (data) => {
      const text = data.toString().trim();
      if (text) {
        logger.debug('Sidecar stderr', { text });
      }
    });

    // Parse JSON lines from stdout
    this.rl = readline.createInterface({
      input: this.process.stdout,
      crlfDelay: Infinity,
    });

    this.rl.on('line', (line) => {
      this._handleMessage(line.trim());
    });

    // Send initialize command
    this._send({ type: 'initialize', config: this.config });
  }

  async stopSidecar() {
    this.shutdownRequested = true;
    if (this.process) {
      this._send({ type: 'shutdown' });

      // Give it 3 seconds to shut down gracefully
      await new Promise((resolve) => {
        const timeout = setTimeout(() => {
          if (this.process) {
            logger.warn('Sidecar did not shut down gracefully, killing');
            this.process.kill('SIGTERM');
          }
          resolve();
        }, 3000);

        if (this.process) {
          this.process.once('exit', () => {
            clearTimeout(timeout);
            resolve();
          });
        } else {
          clearTimeout(timeout);
          resolve();
        }
      });
    }

    this.process = null;
    this.rl = null;
    this.isReady = false;
    this.state = 'idle';
  }

  async waitUntilReady(timeoutMs = 15000) {
    if (this.isReady) return true;

    return new Promise((resolve) => {
      const timeout = setTimeout(() => {
        logger.error('Sidecar ready timeout');
        resolve(false);
      }, timeoutMs);

      const onReady = () => {
        clearTimeout(timeout);
        resolve(true);
      };

      this.once('ready', onReady);
    });
  }

  async startRecording(options = {}) {
    if (!this.isReady) {
      this.emit('error', { code: 'NOT_READY', message: 'Sidecar not ready', recoverable: true });
      return;
    }

    this._send({
      type: 'start_recording',
      device_id: options.deviceId || null,
    });
  }

  async stopRecording() {
    this._send({ type: 'stop_recording' });
  }

  async cancel() {
    this._send({ type: 'cancel' });
  }

  async listDevices() {
    this._send({ type: 'list_devices' });
  }

  getState() {
    return this.state;
  }

  getIsReady() {
    return this.isReady;
  }

  _send(message) {
    if (!this.process || !this.process.stdin.writable) {
      logger.warn('Cannot send message: sidecar not running', { type: message.type });
      return;
    }

    try {
      const json = JSON.stringify(message) + '\n';
      this.process.stdin.write(json);
    } catch (err) {
      logger.error('Failed to send message to sidecar', { error: err.message, type: message.type });
    }
  }

  _handleMessage(line) {
    if (!line) return;

    let msg;
    try {
      msg = JSON.parse(line);
    } catch (err) {
      logger.warn('Invalid JSON from sidecar', { line: line.substring(0, 200) });
      return;
    }

    switch (msg.type) {
      case 'ready':
        this.isReady = true;
        this.restartCount = 0;
        this.state = 'ready';
        logger.info('Sidecar is ready');
        this.emit('ready');
        this.emit('state', 'ready');
        break;

      case 'state':
        this.state = msg.state;
        this.emit('state', msg.state);
        break;

      case 'partial':
        this.emit('partial', msg.text);
        break;

      case 'final':
        this.emit('final', msg.text);
        break;

      case 'vad':
        this.emit('vad', { speech: msg.speech, probability: msg.probability });
        break;

      case 'audio_level':
        this.emit('audio_level', msg.value);
        break;

      case 'devices':
        this.emit('devices', msg.devices);
        break;

      case 'error':
        logger.error('Sidecar error', { code: msg.code, message: msg.message });
        this.emit('error', {
          code: msg.code,
          message: msg.message,
          recoverable: msg.recoverable,
        });
        break;

      case 'recording_stopped':
        this.emit('recording_stopped');
        break;

      default:
        logger.debug('Unknown message type from sidecar', { type: msg.type });
    }
  }

  _handleProcessExit() {
    if (this.shutdownRequested) return;

    if (this.restartCount < this.maxRestarts) {
      const delay = this.restartDelays[this.restartCount] || 2000;
      this.restartCount++;
      logger.info(`Restarting sidecar (attempt ${this.restartCount}/${this.maxRestarts}) in ${delay}ms`);

      setTimeout(() => {
        if (!this.shutdownRequested) {
          this.startSidecar().catch((err) => {
            logger.error('Failed to restart sidecar', { error: err.message });
          });
        }
      }, delay);
    } else {
      logger.error('Sidecar failed after maximum restart attempts');
      this.emit('error', {
        code: 'SIDECAR_CRASHED',
        message: 'Speech sidecar crashed and could not be restarted. Please restart the application.',
        recoverable: false,
      });
    }
  }
}

module.exports = LocalSpeechBridge;
