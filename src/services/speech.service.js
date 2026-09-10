const { EventEmitter } = require('events');
const LocalSpeechBridge = require('./local-speech-bridge.service');
const logger = require('../core/logger').createServiceLogger('SPEECH');
const config = require('../core/config');

class SpeechService extends EventEmitter {
  constructor() {
    super();
    this.bridge = null;
    this.isRecording = false;
    this.sessionStartTime = null;
    this.retryCount = 0;
    this.maxRetries = 3;

    this.initialize();
  }

  async initialize() {
    try {
      const speechConfig = config.get('speech') || {};

      this.bridge = new LocalSpeechBridge({
        language: speechConfig.language || 'en',
        sampleRate: speechConfig.sampleRate || 16000,
        speechStartThreshold: speechConfig.vad?.startThreshold || 0.55,
        speechContinueThreshold: speechConfig.vad?.continueThreshold || 0.40,
        minSpeechMs: speechConfig.vad?.minSpeechMs || 250,
        preRollMs: speechConfig.vad?.preRollMs || 250,
        postRollMs: speechConfig.vad?.postRollMs || 150,
        endpointSilenceMs: speechConfig.vad?.endpointSilenceMs || 900,
        maxUtteranceMs: speechConfig.vad?.maxUtteranceMs || 30000,
      });

      this._setupBridgeEvents();

      await this.bridge.startSidecar();
      const ready = await this.bridge.waitUntilReady(15000);

      if (ready) {
        logger.info('Local speech service initialized successfully');
        this.emit('status', 'Local Speech Services ready');
      } else {
        logger.warn('Speech sidecar did not become ready in time');
        this.emit('status', 'Speech service starting (sidecar loading models)...');
      }
    } catch (error) {
      logger.error('Failed to initialize local speech service', { error: error.message });
      this.emit('error', `Speech recognition unavailable: ${error.message}`);
    }
  }

  _setupBridgeEvents() {
    this.bridge.on('ready', () => {
      logger.info('Speech sidecar ready');
      this.emit('status', 'Local Speech Services ready');
    });

    this.bridge.on('state', (state) => {
      const statusMap = {
        'initializing': 'Initializing speech engine...',
        'ready': 'Local Speech Services ready',
        'listening': 'Listening...',
        'speech_detected': 'Speech detected...',
        'transcribing': 'Transcribing...',
        'endpoint_detected': 'Processing...',
        'finalizing': 'Finalizing transcript...',
        'stopping': 'Stopping...',
        'error': 'Speech error',
      };
      const status = statusMap[state] || state;
      this.emit('status', status);
    });

    this.bridge.on('partial', (text) => {
      if (text && text.trim().length > 0) {
        logger.debug('Interim transcription', { text });
        this.emit('interim-transcription', text);
      }
    });

    this.bridge.on('final', (text) => {
      if (text && text.trim().length > 0) {
        const sessionDuration = this.sessionStartTime ? Date.now() - this.sessionStartTime : 0;
        logger.info('Final transcription received', {
          text,
          sessionDuration: `${sessionDuration}ms`,
          textLength: text.length,
        });
        this.emit('transcription', text);
      }
    });

    this.bridge.on('error', (error) => {
      logger.error('Speech bridge error', { code: error.code, message: error.message });

      const userFriendlyMessages = {
        'MICROPHONE_UNAVAILABLE': 'No microphone found. Please connect a microphone and try again.',
        'MICROPHONE_DISCONNECTED': 'Microphone disconnected. Please reconnect and try again.',
        'SIDECAR_NOT_FOUND': 'Speech engine not found. Please run: npm run speech:build',
        'SIDECAR_CRASHED': 'Speech engine crashed. Please restart the application.',
        'VAD_MODEL_LOAD_FAILED': 'Voice detection model not found. Please run: npm run speech:models',
        'MOONSHINE_START_FAILED': 'Transcription model not found. Please run: npm run speech:models',
      };

      const msg = userFriendlyMessages[error.code] || error.message;
      this.emit('error', msg);

      if (!error.recoverable && this.isRecording) {
        this.isRecording = false;
        this.emit('recording-stopped');
      }
    });

    this.bridge.on('recording_stopped', () => {
      this.isRecording = false;
      this.emit('recording-stopped');
      this.emit('status', 'Recording stopped');
      if (global.windowManager) {
        global.windowManager.handleRecordingStopped();
      }
    });

    this.bridge.on('audio_level', (level) => {
      this.emit('audio-level', level);
    });

    this.bridge.on('vad', (data) => {
      this.emit('vad', data);
    });
  }

  startRecording() {
    try {
      if (!this.bridge || !this.bridge.getIsReady()) {
        const errorMsg = 'Speech service not initialized. Waiting for sidecar...';
        logger.warn(errorMsg);
        this.emit('error', errorMsg);
        return;
      }

      if (this.isRecording) {
        logger.warn('Recording already in progress');
        return;
      }

      this.sessionStartTime = Date.now();
      this.isRecording = true;
      this.retryCount = 0;

      this.emit('recording-started');
      this.bridge.startRecording();

      logger.info('Recording started');
      if (global.windowManager) {
        global.windowManager.handleRecordingStarted();
      }
    } catch (error) {
      logger.error('Failed to start recording', { error: error.message });
      this.emit('error', `Speech recognition failed to start: ${error.message}`);
      this.isRecording = false;
    }
  }

  stopRecording() {
    if (!this.isRecording) {
      return;
    }

    this.isRecording = false;
    const sessionDuration = this.sessionStartTime ? Date.now() - this.sessionStartTime : 0;

    logger.info('Stopping speech recognition', { sessionDuration: `${sessionDuration}ms` });

    if (this.bridge) {
      this.bridge.stopRecording();
    }

    this.emit('recording-stopped');
    this.emit('status', 'Recording stopped');

    if (global.windowManager) {
      global.windowManager.handleRecordingStopped();
    }
  }

  getStatus() {
    return {
      isRecording: this.isRecording,
      isInitialized: this.bridge ? this.bridge.getIsReady() : false,
      sessionDuration: this.sessionStartTime ? Date.now() - this.sessionStartTime : 0,
      retryCount: this.retryCount,
      sidecarState: this.bridge ? this.bridge.getState() : 'not_started',
      config: config.get('speech') || {},
    };
  }

  async testConnection() {
    if (!this.bridge) {
      return { success: false, message: 'Speech bridge not initialized' };
    }

    if (this.bridge.getIsReady()) {
      return { success: true, message: 'Local speech service is ready' };
    }

    return { success: false, message: 'Sidecar not ready. Models may still be loading.' };
  }

  async shutdown() {
    logger.info('Shutting down speech service');
    if (this.isRecording) {
      this.stopRecording();
    }
    if (this.bridge) {
      await this.bridge.stopSidecar();
    }
  }
}

module.exports = new SpeechService();
