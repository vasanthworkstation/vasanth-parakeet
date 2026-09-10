const path = require('path');
const os = require('os');

class ConfigManager {
  constructor() {
    this.env = process.env.NODE_ENV || 'development';
    this.appDataDir = path.join(os.homedir(), '.Vysper');
    this.loadConfiguration();
  }

  loadConfiguration() {
    this.config = {
      app: {
        name: 'Vysper',
        version: '1.0.0',
        processTitle: 'Vysper',
        dataDir: this.appDataDir,
        isDevelopment: this.env === 'development',
        isProduction: this.env === 'production'
      },
      
      window: {
        defaultWidth: 400,
        defaultHeight: 600,
        minWidth: 300,
        minHeight: 400,
        webPreferences: {
          nodeIntegration: false,
          contextIsolation: true,
          enableRemoteModule: false,
          preload: path.join(__dirname, '../../preload.js')
        }
      },

      ocr: {
        language: 'eng',
        tempDir: os.tmpdir(),
        cleanupDelay: 5000
      },

      llm: {
        provider: process.env.LLM_PROVIDER || 'gemini',
        maxRetries: 3,
        timeout: 30000,
        fallbackEnabled: true,
        azure: {
          model: process.env.LLM_MODEL || 'gpt-35-turbo-16k',
          apiKey: process.env.LLM_API_KEY || '',
          baseUrl: process.env.LLM_BASE_URL || '',
          apiVersion: process.env.LLM_API_VERSION || '2024-02-15-preview',
          maxTokens: parseInt(process.env.LLM_MAX_TOKENS) || 4096,
        },
        gemini: {
          model: 'gemini-1.5-flash',
          maxRetries: 3,
          timeout: 30000,
          fallbackEnabled: true,
          enableFallbackMethod: true
        }
      },

      speech: {
        language: 'en',
        sampleRate: 16000,
        vad: {
          startThreshold: 0.55,
          continueThreshold: 0.40,
          minSpeechMs: 250,
          preRollMs: 250,
          postRollMs: 150,
          endpointSilenceMs: 900,
          maxUtteranceMs: 30000,
        },
        moonshine: {
          model: 'streaming-small',
          device: 'cpu',
        },
      },

      session: {
        maxMemorySize: 1000,
        compressionThreshold: 500,
        clearOnRestart: false
      },

      stealth: {
        hideFromDock: true,
        noAttachConsole: true,
        disguiseProcess: true
      }
    };
  }

  get(keyPath) {
    return keyPath.split('.').reduce((obj, key) => obj?.[key], this.config);
  }

  set(keyPath, value) {
    const keys = keyPath.split('.');
    const lastKey = keys.pop();
    const target = keys.reduce((obj, key) => obj[key] = obj[key] || {}, this.config);
    target[lastKey] = value;
  }

  getApiKey(service) {
    const envKey = `${service.toUpperCase()}_API_KEY`;
    return process.env[envKey];
  }

  isFeatureEnabled(feature) {
    return this.get(`features.${feature}`) !== false;
  }
}

module.exports = new ConfigManager(); 