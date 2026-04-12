import OpenAI from "openai";
import type { ChatCompletionMessageParam } from "openai/resources/chat/completions.js";
import {
  ToolExecutionResultSchema,
  stableJsonStringify,
  type AgentTool,
  type ToolContext,
  type ToolExecutionErrorCode,
  type ToolExecutionResult,
  validateToolSchema,
} from "./tools/types.js";

export interface LlmChatResult {
  content: string;
  durationMs: number;
  requestId: string | null;
  model: string;
  toolUsage?: LlmToolUsage;
}

export type LlmChatLikeResult = LlmChatResult | string;

export interface LlmClient<TResult = LlmChatLikeResult> {
  chat(
    model: string,
    messages: ChatCompletionMessageParam[],
    options?: LlmChatOptions,
  ): Promise<TResult>;
}

export interface LlmChatOptions {
  tools?: readonly AgentTool[];
  toolContext?: ToolContext;
  maxToolRounds?: number;
}

export interface LlmToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface LlmToolUsage {
  rounds: number;
  totalCalls: number;
  executedCalls: number;
  deduplicatedCalls: number;
  errorCount: number;
  roundErrors: number[];
  durationMs: number;
  truncated: boolean;
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
    tools?: readonly LlmRequestTool[];
  }) => Promise<LlmRequestResult>;
}

const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_CONSECUTIVE_FAILURES = 3;
const DEFAULT_BACKOFF_MS = 60_000;
export const MAX_TOOL_CALL_ROUNDS = 3;
export const TOOL_LOOP_TRUNCATED_RESPONSE = JSON.stringify({
  commands: [],
  narrations: [],
  reasoning: `tool loop truncated after ${MAX_TOOL_CALL_ROUNDS} rounds`,
});

interface LlmRequestTool {
  type: "function";
  function: {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
  };
}

interface LlmRequestResult {
  content: string;
  requestId: string | null;
  model: string;
  toolCalls?: LlmToolCall[];
}

export function createClient(config: LlmClientConfig): LlmClient<LlmChatResult> {
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
      tools?: readonly LlmRequestTool[];
    }): Promise<LlmRequestResult> => {
      const client = new OpenAI({
        baseURL: config.baseURL,
        apiKey: config.apiKey,
      });
      const response = await client.chat.completions.create(
        {
          model: opts.model,
          messages: opts.messages,
          ...(opts.tools && opts.tools.length > 0 ? { tools: [...opts.tools] } : {}),
        },
        { signal: opts.signal },
      );
      const message = response.choices[0]?.message;

      return {
        content: typeof message?.content === "string" ? message.content : "",
        requestId: readRequestId(response),
        model: response.model ?? opts.model,
        toolCalls: readToolCalls(message?.tool_calls),
      };
    });

  return {
    async chat(
      model: string,
      messages: ChatCompletionMessageParam[],
      options?: LlmChatOptions,
    ): Promise<LlmChatResult> {
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

      const startedAt = Date.now();

      try {
        const result = await runChatLoop({
          model,
          messages,
          options,
          timeoutMs,
          doRequest,
        });
        consecutiveFailures = 0;
        return {
          ...result,
          durationMs: Math.max(0, Date.now() - startedAt),
        };
      } catch (error) {
        consecutiveFailures += 1;
        if (consecutiveFailures >= maxConsecutiveFailures) {
          backoffUntil = now() + backoffMs;
        }
        throw error;
      }
    },
  };
}

export function createMockClient(response = MOCK_LLM_RESPONSE): LlmClient<LlmChatResult> {
  return {
    async chat(model: string): Promise<LlmChatResult> {
      return {
        content: response,
        durationMs: 0,
        requestId: null,
        model,
      };
    },
  };
}

export function normalizeLlmChatResult(result: LlmChatLikeResult, model: string): LlmChatResult {
  if (typeof result === "string") {
    return {
      content: result,
      durationMs: 0,
      requestId: null,
      model,
    };
  }

  return result;
}

function readRequestId(response: { _request_id?: unknown }): string | null {
  return typeof response._request_id === "string" ? response._request_id : null;
}

async function runChatLoop(args: {
  model: string;
  messages: ChatCompletionMessageParam[];
  options?: LlmChatOptions;
  timeoutMs: number;
  doRequest: (options: {
    model: string;
    messages: ChatCompletionMessageParam[];
    signal: AbortSignal;
    tools?: readonly LlmRequestTool[];
  }) => Promise<LlmRequestResult>;
}): Promise<Omit<LlmChatResult, "durationMs">> {
  const { model, messages, options, timeoutMs, doRequest } = args;
  const tools = normalizeTools(options?.tools);
  const maxToolRounds = normalizeMaxToolRounds(options?.maxToolRounds);

  if (tools.length === 0) {
    return await requestWithTimeout({ model, messages, timeoutMs, doRequest });
  }

  if (!options?.toolContext) {
    throw new Error("toolContext is required when tools are provided");
  }

  const requestTools = tools.map(toLlmRequestTool);
  const requestMessages = [...messages];
  const toolUsage = createEmptyToolUsage();
  const toolCache = new Map<string, ToolExecutionResult>();
  const toolsByName = new Map(tools.map((tool) => [tool.name, tool]));

  while (true) {
    const response = await requestWithTimeout({
      model,
      messages: requestMessages,
      timeoutMs,
      doRequest,
      tools: requestTools,
    });

    if (!response.toolCalls || response.toolCalls.length === 0) {
      return {
        ...response,
        toolUsage,
      };
    }

    if (toolUsage.rounds >= maxToolRounds) {
      return {
        content: TOOL_LOOP_TRUNCATED_RESPONSE,
        requestId: response.requestId,
        model: response.model,
        toolUsage: {
          ...toolUsage,
          truncated: true,
        },
      };
    }

    toolUsage.rounds += 1;
    requestMessages.push(toAssistantToolCallMessage(response));

    const roundStartedAt = Date.now();
    const roundResults: ExecutedToolCall[] = [];
    for (const toolCall of response.toolCalls) {
      roundResults.push(
        await executeToolCall({
          toolCall,
          toolsByName,
          toolContext: options.toolContext,
          toolCache,
        }),
      );
    }

    const roundDurationMs = Math.max(0, Date.now() - roundStartedAt);
    const roundErrorCount = roundResults.filter((entry) => entry.result.status === "error").length;

    toolUsage.durationMs += roundDurationMs;
    toolUsage.totalCalls += roundResults.length;
    toolUsage.executedCalls += roundResults.filter((entry) => entry.executed).length;
    toolUsage.deduplicatedCalls += roundResults.filter((entry) => entry.result.deduplicated).length;
    toolUsage.errorCount += roundErrorCount;
    toolUsage.roundErrors.push(roundErrorCount);

    requestMessages.push(...roundResults.map((entry) => toToolMessage(entry.result)));
  }
}

async function requestWithTimeout(args: {
  model: string;
  messages: ChatCompletionMessageParam[];
  timeoutMs: number;
  doRequest: (options: {
    model: string;
    messages: ChatCompletionMessageParam[];
    signal: AbortSignal;
    tools?: readonly LlmRequestTool[];
  }) => Promise<LlmRequestResult>;
  tools?: readonly LlmRequestTool[];
}): Promise<LlmRequestResult> {
  const { model, messages, timeoutMs, doRequest, tools } = args;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  try {
    return await doRequest({
      model,
      messages,
      signal: controller.signal,
      tools,
    });
  } catch (error) {
    if (controller.signal.aborted) {
      throw new LlmTimeoutError(timeoutMs);
    }

    throw error;
  } finally {
    clearTimeout(timer);
  }
}

function normalizeTools(tools: readonly AgentTool[] | undefined): AgentTool[] {
  if (!tools || tools.length === 0) {
    return [];
  }

  const names = new Set<string>();
  for (const tool of tools) {
    if (tool.readonly !== true) {
      throw new Error(`tool '${tool.name}' must be readonly in the current runtime`);
    }

    if (names.has(tool.name)) {
      throw new Error(`duplicate tool name '${tool.name}' is not allowed`);
    }

    names.add(tool.name);
  }

  return [...tools];
}

function normalizeMaxToolRounds(value: number | undefined): number {
  if (!Number.isFinite(value)) {
    return MAX_TOOL_CALL_ROUNDS;
  }

  return Math.max(0, Math.floor(value));
}

function toLlmRequestTool(tool: AgentTool): LlmRequestTool {
  return {
    type: "function",
    function: {
      name: tool.name,
      description: tool.description,
      parameters: tool.parameters as unknown as Record<string, unknown>,
    },
  };
}

interface ExecutedToolCall {
  executed: boolean;
  result: ToolExecutionResult;
}

async function executeToolCall(args: {
  toolCall: LlmToolCall;
  toolsByName: Map<string, AgentTool>;
  toolContext: ToolContext;
  toolCache: Map<string, ToolExecutionResult>;
}): Promise<ExecutedToolCall> {
  const { toolCall, toolsByName, toolContext, toolCache } = args;
  const tool = toolsByName.get(toolCall.name);
  const parsedArguments = parseToolArguments(toolCall.arguments);
  const fingerprint = createToolCallFingerprint(toolCall.name, parsedArguments.fingerprintValue);
  const cachedResult = toolCache.get(fingerprint);

  if (cachedResult) {
    return {
      executed: false,
      result: {
        ...cachedResult,
        callId: toolCall.id,
        deduplicated: true,
      },
    };
  }

  if (!tool) {
    const result = createToolErrorResult(toolCall, "TOOL_NOT_FOUND", `tool '${toolCall.name}' is not registered`);
    toolCache.set(fingerprint, result);
    return { executed: false, result };
  }

  if (!parsedArguments.ok) {
    const result = createToolErrorResult(
      toolCall,
      "INVALID_TOOL_ARGS",
      `tool '${tool.name}' arguments must be valid JSON`,
      [parsedArguments.message],
    );
    toolCache.set(fingerprint, result);
    return { executed: false, result };
  }

  const argsValidation = validateToolSchema(tool.parameters, parsedArguments.value);
  if (!argsValidation.ok) {
    const result = createToolErrorResult(
      toolCall,
      "INVALID_TOOL_ARGS",
      `tool '${tool.name}' arguments failed schema validation`,
      argsValidation.errors,
    );
    toolCache.set(fingerprint, result);
    return { executed: false, result };
  }

  try {
    const output = await tool.execute(parsedArguments.value, toolContext);
    const resultValidation = validateToolSchema(tool.result, output);
    if (!resultValidation.ok) {
      const result = createToolErrorResult(
        toolCall,
        "INVALID_TOOL_RESULT",
        `tool '${tool.name}' returned invalid output`,
        resultValidation.errors,
      );
      toolCache.set(fingerprint, result);
      return { executed: true, result };
    }

    const result = validateToolExecutionEnvelope({
      toolName: tool.name,
      callId: toolCall.id,
      status: "ok",
      deduplicated: false,
      output,
    });
    toolCache.set(fingerprint, result);
    return { executed: true, result };
  } catch (error) {
    const result = createToolErrorResult(
      toolCall,
      "TOOL_EXECUTION_FAILED",
      `tool '${tool.name}' execution failed`,
      [error instanceof Error ? error.message : String(error)],
    );
    toolCache.set(fingerprint, result);
    return { executed: true, result };
  }
}

function parseToolArguments(rawArguments: string):
  | { ok: true; value: unknown; fingerprintValue: unknown }
  | { ok: false; message: string; fingerprintValue: unknown } {
  try {
    const value = JSON.parse(rawArguments) as unknown;
    return {
      ok: true,
      value,
      fingerprintValue: value,
    };
  } catch (error) {
    return {
      ok: false,
      message: error instanceof Error ? error.message : "invalid JSON",
      fingerprintValue: { __raw: rawArguments },
    };
  }
}

function createToolCallFingerprint(toolName: string, args: unknown): string {
  return `${toolName}:${stableJsonStringify(args)}`;
}

function toAssistantToolCallMessage(response: LlmRequestResult): ChatCompletionMessageParam {
  return {
    role: "assistant",
    content: response.content,
    tool_calls: (response.toolCalls ?? []).map((toolCall) => ({
      id: toolCall.id,
      type: "function",
      function: {
        name: toolCall.name,
        arguments: toolCall.arguments,
      },
    })),
  } as ChatCompletionMessageParam;
}

function toToolMessage(result: ToolExecutionResult): ChatCompletionMessageParam {
  return {
    role: "tool",
    tool_call_id: result.callId,
    content: stableJsonStringify(result),
  } as ChatCompletionMessageParam;
}

function createToolErrorResult(
  toolCall: LlmToolCall,
  code: ToolExecutionErrorCode,
  message: string,
  details: string[] = [],
): ToolExecutionResult {
  return validateToolExecutionEnvelope({
    toolName: toolCall.name,
    callId: toolCall.id,
    status: "error",
    deduplicated: false,
    error: {
      code,
      message,
      details,
    },
  });
}

function validateToolExecutionEnvelope(result: ToolExecutionResult): ToolExecutionResult {
  const validation = validateToolSchema(ToolExecutionResultSchema, result);
  if (validation.ok) {
    return result;
  }

  return {
    toolName: result.toolName,
    callId: result.callId,
    status: "error",
    deduplicated: false,
    error: {
      code: "TOOL_EXECUTION_FAILED",
      message: "tool result envelope validation failed",
      details: validation.errors,
    },
  };
}

function createEmptyToolUsage(): LlmToolUsage {
  return {
    rounds: 0,
    totalCalls: 0,
    executedCalls: 0,
    deduplicatedCalls: 0,
    errorCount: 0,
    roundErrors: [],
    durationMs: 0,
    truncated: false,
  };
}

function readToolCalls(responseToolCalls: unknown): LlmToolCall[] | undefined {
  if (!Array.isArray(responseToolCalls) || responseToolCalls.length === 0) {
    return undefined;
  }

  const toolCalls = responseToolCalls
    .map((entry, index) => {
      const candidate = entry as {
        id?: unknown;
        function?: {
          name?: unknown;
          arguments?: unknown;
        };
      };
      const name = candidate.function?.name;
      const args = candidate.function?.arguments;
      if (typeof name !== "string" || typeof args !== "string") {
        return null;
      }

      return {
        id: typeof candidate.id === "string" ? candidate.id : `tool_call_${index + 1}`,
        name,
        arguments: args,
      } satisfies LlmToolCall;
    })
    .filter((entry): entry is LlmToolCall => entry !== null);

  return toolCalls.length > 0 ? toolCalls : undefined;
}
