import OpenAI from "openai";
import type { ChatCompletionMessageParam } from "openai/resources/chat/completions.js";

export interface LlmClient {
  chat(model: string, messages: ChatCompletionMessageParam[]): Promise<string>;
}

export const MOCK_LLM_RESPONSE = JSON.stringify({
  commands: [],
  narrations: [],
  reasoning: "mock deterministic noop",
});

export class LlmTimeoutError extends Error {
  constructor(public readonly timeoutMs: number) {
    super(`LLM call timed out after ${timeoutMs}ms`);
    this.name = "LlmTimeoutError";
  }
}

export class LlmBackoffError extends Error {
  constructor(public readonly retryAt: number) {
    super(`LLM in backoff, retry at ${retryAt}`);
    this.name = "LlmBackoffError";
  }
}

export interface LlmClientConfig {
  baseURL: string;
  apiKey: string;
  model: string;
  timeoutMs?: number;
  maxConsecutiveFailures?: number;
  backoffMs?: number;
  now?: () => number;
  chatCompletionRequest?: (options: {
    model: string;
    messages: ChatCompletionMessageParam[];
    signal: AbortSignal;
  }) => Promise<string>;
}

const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_CONSECUTIVE_FAILURES = 3;
const DEFAULT_BACKOFF_MS = 60_000;

export function createClient(config: LlmClientConfig): LlmClient {
  const timeoutMs = config.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const maxConsecutiveFailures =
    config.maxConsecutiveFailures ?? DEFAULT_MAX_CONSECUTIVE_FAILURES;
  const backoffMs = config.backoffMs ?? DEFAULT_BACKOFF_MS;
  const now = config.now ?? (() => Date.now());

  let consecutiveFailures = 0;
  let backoffUntil = 0;

  const doRequest =
    config.chatCompletionRequest ??
    (async (opts: {
      model: string;
      messages: ChatCompletionMessageParam[];
      signal: AbortSignal;
    }) => {
      const client = new OpenAI({
        baseURL: config.baseURL,
        apiKey: config.apiKey,
      });
      const response = await client.chat.completions.create(
        { model: opts.model, messages: opts.messages },
        { signal: opts.signal },
      );
      return response.choices[0]?.message?.content ?? "";
    });

  return {
    async chat(
      model: string,
      messages: ChatCompletionMessageParam[],
    ): Promise<string> {
      const currentTime = now();

      if (
        consecutiveFailures >= maxConsecutiveFailures &&
        currentTime < backoffUntil
      ) {
        throw new LlmBackoffError(backoffUntil);
      }

      if (consecutiveFailures >= maxConsecutiveFailures) {
        consecutiveFailures = 0;
      }

      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), timeoutMs);

      try {
        const result = await doRequest({
          model,
          messages,
          signal: controller.signal,
        });
        clearTimeout(timer);
        consecutiveFailures = 0;
        return result;
      } catch (error) {
        clearTimeout(timer);
        if (controller.signal.aborted) {
          consecutiveFailures += 1;
          if (consecutiveFailures >= maxConsecutiveFailures) {
            backoffUntil = now() + backoffMs;
          }
          throw new LlmTimeoutError(timeoutMs);
        }
        consecutiveFailures += 1;
        if (consecutiveFailures >= maxConsecutiveFailures) {
          backoffUntil = now() + backoffMs;
        }
        throw error;
      }
    },
  };
}

export function createMockClient(response = MOCK_LLM_RESPONSE): LlmClient {
  return {
    async chat(): Promise<string> {
      return response;
    },
  };
}
