import OpenAI from "openai";
import type { ChatCompletionMessageParam } from "openai/resources/chat/completions.js";

export interface LlmConfig {
  baseURL: string;
  apiKey: string;
  model: string;
}

export interface LlmClient {
  chat(messages: ChatCompletionMessageParam[], model: string): Promise<string>;
}

class OpenAiLlmClient implements LlmClient {
  private readonly client: OpenAI;

  constructor(config: LlmConfig) {
    this.client = new OpenAI({
      baseURL: config.baseURL,
      apiKey: config.apiKey,
    });
  }

  async chat(messages: ChatCompletionMessageParam[], model: string): Promise<string> {
    const response = await this.client.chat.completions.create({
      model,
      messages,
    });
    return response.choices[0]?.message?.content ?? "";
  }
}

const DEFAULT_MOCK_RESPONSE = JSON.stringify({
  commands: [],
  narrations: [],
  reasoning: "mock response",
});

export function createClient(config: LlmConfig): LlmClient {
  return new OpenAiLlmClient(config);
}

export function createMockClient(response = DEFAULT_MOCK_RESPONSE): LlmClient {
  return {
    async chat(): Promise<string> {
      return response;
    },
  };
}
