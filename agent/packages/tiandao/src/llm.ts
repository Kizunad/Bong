import OpenAI from "openai";
import type { ChatCompletionMessageParam } from "openai/resources/chat/completions.js";

export const DEFAULT_LLM_TIMEOUT_MS = 30_000;
export const DEFAULT_LLM_MAX_CONSECUTIVE_FAILURES = 3;
export const DEFAULT_LLM_BACKOFF_MS = 60_000;

export interface LlmConfig {
  baseURL: string;
  apiKey: string;
  model: string;
  timeoutMs?: number;
  maxConsecutiveFailures?: number;
  backoffMs?: number;
  now?: () => number;
  chatCompletionRequest?: (args: {
    model: string;
    messages: LlmMessage[];
    signal: AbortSignal;
  }) => Promise<string>;
}

export interface LlmMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface LlmClient {
  chat(model: string, messages: LlmMessage[]): Promise<string>;
}

export interface LlmGuardState {
  consecutiveFailures: number;
  backoffUntil: number;
}

export interface GuardedLlmClient extends LlmClient {
  getGuardState(): LlmGuardState;
}

export class LlmTimeoutError extends Error {
  constructor(timeoutMs: number) {
    super(`[llm] request timed out after ${timeoutMs}ms`);
    this.name = "LlmTimeoutError";
  }
}

export class LlmBackoffError extends Error {
  readonly retryAt: number;

  constructor(retryAt: number) {
    super("[llm] request skipped due to temporary backoff");
    this.name = "LlmBackoffError";
    this.retryAt = retryAt;
  }
}

export const MOCK_LLM_RESPONSE = JSON.stringify({
  commands: [],
  narrations: [],
  reasoning: "mock deterministic noop",
});

export function createClient(config: LlmConfig): LlmClient {
  const timeoutMs = config.timeoutMs ?? DEFAULT_LLM_TIMEOUT_MS;
  const maxConsecutiveFailures =
    config.maxConsecutiveFailures ?? DEFAULT_LLM_MAX_CONSECUTIVE_FAILURES;
  const backoffMs = config.backoffMs ?? DEFAULT_LLM_BACKOFF_MS;
  const now = config.now ?? (() => Date.now());

  let consecutiveFailures = 0;
  let backoffUntil = 0;

  const openai = config.chatCompletionRequest
    ? undefined
    : new OpenAI({
        baseURL: config.baseURL,
        apiKey: config.apiKey,
      });

  const requestChatCompletion = config.chatCompletionRequest;

  const defaultRequestChatCompletion = async ({
    model,
    messages,
    signal,
  }: {
    model: string;
    messages: LlmMessage[];
    signal: AbortSignal;
  }): Promise<string> => {
    if (!openai) {
      throw new Error("[llm] missing chat completion request implementation");
    }

    const response = await openai.chat.completions.create(
      {
        model,
        messages: messages as ChatCompletionMessageParam[],
      },
      {
        signal,
      },
    );

    return response.choices[0]?.message?.content ?? "";
  };

  const registerFailure = (): void => {
    consecutiveFailures += 1;
    if (consecutiveFailures >= maxConsecutiveFailures) {
      backoffUntil = now() + backoffMs;
    }
  };

  const guardedClient: GuardedLlmClient = {
    async chat(model: string, messages: LlmMessage[]): Promise<string> {
      const nowTs = now();
      if (backoffUntil > nowTs) {
        throw new LlmBackoffError(backoffUntil);
      }

      const controller = new AbortController();
      const timeout = setTimeout(() => {
        controller.abort();
      }, timeoutMs);

      let timedOut = false;
      const abortHandler = () => {
        timedOut = true;
      };
      controller.signal.addEventListener("abort", abortHandler, { once: true });

      try {
        const content = await (requestChatCompletion ?? defaultRequestChatCompletion)({
          model,
          messages,
          signal: controller.signal,
        });
        consecutiveFailures = 0;
        backoffUntil = 0;
        return content;
      } catch (error) {
        registerFailure();

        if (timedOut) {
          throw new LlmTimeoutError(timeoutMs);
        }

        throw error;
      } finally {
        clearTimeout(timeout);
      }
    },
    getGuardState(): LlmGuardState {
      return {
        consecutiveFailures,
        backoffUntil,
      };
    },
  };

  return guardedClient;
}

export function createMockClient(response: string = MOCK_LLM_RESPONSE): LlmClient {
  return {
    async chat(): Promise<string> {
      return response;
    },
  };
}
