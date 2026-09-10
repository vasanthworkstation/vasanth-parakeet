const https = require('https');
const logger = require('../core/logger').createServiceLogger('LLM');
const config = require('../core/config');
const { promptLoader } = require('../../prompt-loader');

class LLMService {
  constructor() {
    this.provider = null;
    this.client = null;
    this.model = null;
    this.isInitialized = false;
    this.requestCount = 0;
    this.errorCount = 0;

    this.initializeClient();
  }

  initializeClient() {
    this.provider = config.get('llm.provider') || 'gemini';
    logger.info('LLM provider configured', { provider: this.provider });

    if (this.provider === 'azure') {
      this._initAzure();
    } else {
      this._initGemini();
    }
  }

  _initAzure() {
    const azureConfig = config.get('llm.azure');
    if (!azureConfig.apiKey || !azureConfig.baseUrl) {
      logger.warn('Azure OpenAI not configured', {
        hasKey: !!azureConfig.apiKey,
        hasUrl: !!azureConfig.baseUrl,
      });
      return;
    }

    this.azureConfig = azureConfig;
    this.isInitialized = true;
    logger.info('Azure OpenAI client initialized', {
      model: azureConfig.model,
      baseUrl: azureConfig.baseUrl,
      apiVersion: azureConfig.apiVersion,
    });
  }

  _initGemini() {
    const apiKey = config.getApiKey('GEMINI');
    if (!apiKey || apiKey === 'your-api-key-here') {
      logger.warn('Gemini API key not configured');
      return;
    }

    try {
      const { GoogleGenerativeAI } = require('@google/generative-ai');
      this.client = new GoogleGenerativeAI(apiKey);
      this.model = this.client.getGenerativeModel({
        model: config.get('llm.gemini.model'),
      });
      this.isInitialized = true;
      logger.info('Gemini AI client initialized', {
        model: config.get('llm.gemini.model'),
      });
    } catch (error) {
      logger.error('Failed to initialize Gemini client', { error: error.message });
    }
  }

  async processTextWithSkill(text, activeSkill, sessionMemory = [], programmingLanguage = null) {
    if (!this.isInitialized) {
      throw new Error(`LLM service not initialized. Check ${this.provider} configuration.`);
    }

    const startTime = Date.now();
    this.requestCount++;

    try {
      logger.info('Processing text with LLM', {
        provider: this.provider,
        activeSkill,
        textLength: text.length,
        programmingLanguage: programmingLanguage || 'not specified',
        requestId: this.requestCount,
      });

      const messages = this._buildMessages(text, activeSkill, sessionMemory, programmingLanguage);
      const response = await this._executeWithRetries(messages);

      logger.logPerformance('LLM text processing', startTime, {
        activeSkill,
        responseLength: response.length,
        requestId: this.requestCount,
      });

      return {
        response,
        metadata: {
          skill: activeSkill,
          programmingLanguage,
          processingTime: Date.now() - startTime,
          requestId: this.requestCount,
          provider: this.provider,
          usedFallback: false,
        },
      };
    } catch (error) {
      this.errorCount++;
      logger.error('LLM processing failed', { error: error.message, activeSkill, requestId: this.requestCount });

      if (config.get('llm.fallbackEnabled')) {
        return this.generateFallbackResponse(text, activeSkill);
      }
      throw error;
    }
  }

  async processTranscriptionWithIntelligentResponse(text, activeSkill, sessionMemory = [], programmingLanguage = null) {
    if (!this.isInitialized) {
      throw new Error(`LLM service not initialized. Check ${this.provider} configuration.`);
    }

    const startTime = Date.now();
    this.requestCount++;

    try {
      logger.info('Processing transcription with intelligent response', {
        provider: this.provider,
        activeSkill,
        textLength: text.length,
        requestId: this.requestCount,
      });

      const systemPrompt = this._getIntelligentTranscriptionPrompt(activeSkill, programmingLanguage);
      const messages = this._buildMessages(text, activeSkill, sessionMemory, programmingLanguage, systemPrompt);
      const response = await this._executeWithRetries(messages);

      logger.logPerformance('LLM transcription processing', startTime, {
        activeSkill,
        responseLength: response.length,
        requestId: this.requestCount,
      });

      return {
        response,
        metadata: {
          skill: activeSkill,
          programmingLanguage,
          processingTime: Date.now() - startTime,
          requestId: this.requestCount,
          provider: this.provider,
          usedFallback: false,
          isTranscriptionResponse: true,
        },
      };
    } catch (error) {
      this.errorCount++;
      logger.error('LLM transcription processing failed', { error: error.message, activeSkill, requestId: this.requestCount });

      if (config.get('llm.fallbackEnabled')) {
        return this._generateIntelligentFallbackResponse(text, activeSkill);
      }
      throw error;
    }
  }

  _buildMessages(text, activeSkill, sessionMemory, programmingLanguage, overrideSystemPrompt = null) {
    const messages = [];

    // System prompt
    let systemPrompt = overrideSystemPrompt;
    if (!systemPrompt) {
      try {
        const sessionManager = require('../managers/session.manager');
        if (sessionManager && typeof sessionManager.getSkillContext === 'function') {
          const skillContext = sessionManager.getSkillContext(activeSkill, programmingLanguage);
          systemPrompt = skillContext.skillPrompt;
        }
      } catch (e) {
        // Fall through
      }

      if (!systemPrompt) {
        try {
          const requestComponents = promptLoader.getRequestComponents(activeSkill, text, sessionMemory, programmingLanguage);
          systemPrompt = requestComponents.skillPrompt;
        } catch (e) {
          // Fall through
        }
      }
    }

    if (systemPrompt) {
      messages.push({ role: 'system', content: systemPrompt });
    }

    // Conversation history
    try {
      const sessionManager = require('../managers/session.manager');
      if (sessionManager && typeof sessionManager.getConversationHistory === 'function') {
        const history = sessionManager.getConversationHistory(15);
        for (const event of history) {
          if (event.role === 'system' || !event.content || !event.content.trim()) continue;
          messages.push({
            role: event.role === 'model' ? 'assistant' : 'user',
            content: event.content.trim(),
          });
        }
      }
    } catch (e) {
      // No session history available
    }

    // Current user message
    messages.push({
      role: 'user',
      content: `Context: ${activeSkill.toUpperCase()} analysis request\n\nText to analyze:\n${text}`,
    });

    return messages;
  }

  async _executeWithRetries(messages) {
    const maxRetries = config.get('llm.maxRetries') || config.get('llm.gemini.maxRetries') || 3;
    const timeout = config.get('llm.timeout') || config.get('llm.gemini.timeout') || 30000;

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        let response;
        if (this.provider === 'azure') {
          response = await this._executeAzure(messages, timeout);
        } else {
          response = await this._executeGemini(messages, timeout);
        }
        return response;
      } catch (error) {
        logger.warn(`LLM attempt ${attempt} failed`, {
          error: error.message,
          provider: this.provider,
          remainingAttempts: maxRetries - attempt,
        });

        if (attempt === maxRetries) {
          throw new Error(`LLM failed after ${maxRetries} attempts: ${error.message}`);
        }

        const delay = 1000 * attempt + Math.random() * 1000;
        await new Promise(resolve => setTimeout(resolve, delay));
      }
    }
  }

  async _executeAzure(messages, timeout) {
    const cfg = this.azureConfig;
    let baseUrl = cfg.baseUrl.replace(/\/$/, '');
    const url = `${baseUrl}/openai/deployments/${cfg.model}/chat/completions?api-version=${cfg.apiVersion}`;

    const body = JSON.stringify({
      messages: messages.map(m => ({ role: m.role, content: m.content })),
      max_tokens: cfg.maxTokens,
      temperature: 0.7,
      top_p: 0.95,
    });

    return new Promise((resolve, reject) => {
      const parsedUrl = new URL(url);
      const options = {
        hostname: parsedUrl.hostname,
        port: 443,
        path: parsedUrl.pathname + parsedUrl.search,
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'api-key': cfg.apiKey,
          'Content-Length': Buffer.byteLength(body),
        },
        timeout: timeout,
      };

      const req = https.request(options, (res) => {
        let data = '';
        res.on('data', chunk => data += chunk);
        res.on('end', () => {
          try {
            if (res.statusCode !== 200) {
              reject(new Error(`Azure OpenAI HTTP ${res.statusCode}: ${data}`));
              return;
            }

            const parsed = JSON.parse(data);
            const text = parsed.choices?.[0]?.message?.content;

            if (!text || !text.trim()) {
              reject(new Error('Empty response from Azure OpenAI'));
              return;
            }

            logger.debug('Azure OpenAI response received', {
              statusCode: res.statusCode,
              responseLength: text.length,
              model: parsed.model,
              usage: parsed.usage,
            });

            resolve(text.trim());
          } catch (parseError) {
            reject(new Error(`Failed to parse Azure response: ${parseError.message}`));
          }
        });
      });

      req.on('error', error => reject(new Error(`Azure OpenAI request failed: ${error.message}`)));
      req.on('timeout', () => { req.destroy(); reject(new Error('Azure OpenAI request timeout')); });
      req.write(body);
      req.end();
    });
  }

  async _executeGemini(messages, timeout) {
    if (!this.model) {
      throw new Error('Gemini model not initialized');
    }

    const systemMessages = messages.filter(m => m.role === 'system');
    const nonSystemMessages = messages.filter(m => m.role !== 'system');

    const geminiRequest = {
      contents: nonSystemMessages.map(m => ({
        role: m.role === 'assistant' ? 'model' : 'user',
        parts: [{ text: m.content }],
      })),
      generationConfig: {
        temperature: 0.7,
        maxOutputTokens: 2048,
        topK: 40,
        topP: 0.95,
      },
    };

    if (systemMessages.length > 0) {
      geminiRequest.systemInstruction = {
        parts: [{ text: systemMessages.map(m => m.content).join('\n\n') }],
      };
    }

    const timeoutPromise = new Promise((_, reject) =>
      setTimeout(() => reject(new Error('Request timeout')), timeout)
    );

    const result = await Promise.race([
      this.model.generateContent(geminiRequest),
      timeoutPromise,
    ]);

    if (!result.response) {
      throw new Error('Empty response from Gemini API');
    }

    const text = result.response.text();
    if (!text || !text.trim()) {
      throw new Error('Empty text in Gemini response');
    }

    return text.trim();
  }

  _getIntelligentTranscriptionPrompt(activeSkill, programmingLanguage) {
    let prompt = `# Intelligent Transcription Response System

Assume you are asked a question in ${activeSkill.toUpperCase()} mode. Your job is to intelligently respond to question/message with appropriate brevity.
Assume you are in an interview and you need to perform best in ${activeSkill.toUpperCase()} mode.
Always respond to the point, do not repeat the question or unnecessary information which is not related to ${activeSkill}.`;

    if (programmingLanguage) {
      prompt += `\n\nCODING CONTEXT: When providing code examples or technical solutions, use ${programmingLanguage.toUpperCase()} as the primary programming language.`;
    }

    prompt += `

## Response Rules:

### If the transcription is casual conversation, greetings, or NOT related to ${activeSkill}:
- Respond with: "Yeah, I'm listening. Ask your question relevant to ${activeSkill}."
- Or similar brief acknowledgments like: "I'm here, what's your ${activeSkill} question?"

### If the transcription IS relevant to ${activeSkill} or is a follow-up question:
- Provide a comprehensive, detailed response
- Use bullet points, examples, and explanations
- Focus on actionable insights and complete answers
- Do not truncate or shorten your response

## Response Format:
- Keep responses detailed
- Use bullet points for structured answers
- Be encouraging and helpful
- Stay focused on ${activeSkill}

Remember: Be intelligent about filtering - only provide detailed responses when the user actually needs help with ${activeSkill}.`;

    return prompt;
  }

  generateFallbackResponse(text, activeSkill) {
    logger.info('Generating fallback response', { activeSkill });

    const fallbackResponses = {
      'dsa': 'This appears to be a data structures and algorithms problem. Consider breaking it down into smaller components and identifying the appropriate algorithm or data structure to use.',
      'system-design': 'For this system design question, consider scalability, reliability, and the trade-offs between different architectural approaches.',
      'programming': 'This looks like a programming challenge. Focus on understanding the requirements, edge cases, and optimal time/space complexity.',
      'default': `I can help analyze this content. Please ensure your ${this.provider === 'azure' ? 'Azure OpenAI' : 'Gemini'} configuration is correct for detailed analysis.`,
    };

    return {
      response: fallbackResponses[activeSkill] || fallbackResponses.default,
      metadata: {
        skill: activeSkill,
        processingTime: 0,
        requestId: this.requestCount,
        usedFallback: true,
      },
    };
  }

  _generateIntelligentFallbackResponse(text, activeSkill) {
    const skillKeywords = {
      'dsa': ['algorithm', 'data structure', 'array', 'tree', 'graph', 'sort', 'search', 'complexity', 'big o'],
      'programming': ['code', 'function', 'variable', 'class', 'method', 'bug', 'debug', 'syntax'],
      'system-design': ['scalability', 'database', 'architecture', 'microservice', 'load balancer', 'cache'],
      'behavioral': ['interview', 'experience', 'situation', 'leadership', 'conflict', 'team'],
      'sales': ['customer', 'deal', 'negotiation', 'price', 'revenue', 'prospect'],
      'data-science': ['data', 'model', 'machine learning', 'statistics', 'analytics', 'python', 'pandas'],
      'devops': ['deployment', 'ci/cd', 'docker', 'kubernetes', 'infrastructure', 'monitoring'],
    };

    const textLower = text.toLowerCase();
    const relevantKeywords = skillKeywords[activeSkill] || [];
    const hasRelevantKeywords = relevantKeywords.some(kw => textLower.includes(kw));
    const questionIndicators = ['how', 'what', 'why', 'when', 'where', 'can you', 'could you', '?'];
    const seemsLikeQuestion = questionIndicators.some(q => textLower.includes(q));

    const response = (hasRelevantKeywords || seemsLikeQuestion)
      ? `I'm having trouble processing that right now, but it sounds like a ${activeSkill} question. Could you rephrase?`
      : `Yeah, I'm listening. Ask your question relevant to ${activeSkill}.`;

    return {
      response,
      metadata: {
        skill: activeSkill,
        processingTime: 0,
        requestId: this.requestCount,
        usedFallback: true,
        isTranscriptionResponse: true,
      },
    };
  }

  async testConnection() {
    if (!this.isInitialized) {
      return { success: false, error: 'Service not initialized' };
    }

    try {
      const messages = [
        { role: 'user', content: 'Test connection. Please respond with "OK".' },
      ];

      const startTime = Date.now();
      const response = await this._executeWithRetries(messages);
      const latency = Date.now() - startTime;

      logger.info('Connection test successful', { provider: this.provider, response, latency });
      return { success: true, response: response.trim(), latency, provider: this.provider };
    } catch (error) {
      logger.error('Connection test failed', { error: error.message, provider: this.provider });
      return { success: false, error: error.message, provider: this.provider };
    }
  }

  updateApiKey(newApiKey) {
    if (this.provider === 'azure') {
      process.env.LLM_API_KEY = newApiKey;
      this.azureConfig.apiKey = newApiKey;
    } else {
      process.env.GEMINI_API_KEY = newApiKey;
    }
    this.isInitialized = false;
    this.initializeClient();
    logger.info('API key updated and client reinitialized', { provider: this.provider });
  }

  getStats() {
    return {
      isInitialized: this.isInitialized,
      provider: this.provider,
      requestCount: this.requestCount,
      errorCount: this.errorCount,
      successRate: this.requestCount > 0 ? ((this.requestCount - this.errorCount) / this.requestCount) * 100 : 0,
      config: this.provider === 'azure'
        ? { model: this.azureConfig?.model, baseUrl: this.azureConfig?.baseUrl }
        : config.get('llm.gemini'),
    };
  }
}

module.exports = new LLMService();
