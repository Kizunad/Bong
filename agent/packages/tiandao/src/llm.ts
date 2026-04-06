import OpenAI from "openai";
import type { ChatCompletionMessageParam } from "openai/resources/chat/completions.js";

export interface LlmConfig {
  baseURL: string;
  apiKey: string;
  model: string;
}

export function createClient(config: LlmConfig): OpenAI {
  return new OpenAI({
    baseURL: config.baseURL,
    apiKey: config.apiKey,
  });
}

export async function chat(
  client: OpenAI,
  model: string,
  messages: ChatCompletionMessageParam[],
): Promise<string> {
  const response = await client.chat.completions.create({
    model,
    messages,
  });
  return response.choices[0]?.message?.content ?? "";
}
