import { fileURLToPath } from "node:url";
import Redis from "ioredis";
import type { Command, Narration } from "@bong/schema";
import { DeathInsightRuntime } from "./death-insight-runtime.js";
import { HeartDemonRuntime } from "./heart-demon-runtime.js";
import { InsightRuntime } from "./insight-runtime.js";
import { SkillLvUpNarrationRuntime } from "./skill-lv-up-runtime.js";
import { TribulationNarrationRuntime } from "./tribulation-runtime.js";
import { WoliuNarrationRuntime } from "./woliu-narration.js";
import { ZhenmaiNarrationRuntime } from "./zhenmai-narration.js";
import { createClient as createLlmClient, createMockClient, type LlmClient } from "./llm.js";
import { createMockWorldState } from "./mock-state.js";
import {
  createDefaultAgents,
  loadEnv,
  resolveRuntimeConfig,
  runRuntime,
  runTick,
  type TickPublishMetadata,
} from "./runtime.js";
import { WorldModel } from "./world-model.js";

const MOCK_COMPLETION_MARKER = "[tiandao] mock tick complete";

export interface PublishSink {
  publishCommands(source: string, commands: Command[], metadata?: TickPublishMetadata): Promise<void>;
  publishNarrations(narrations: Narration[], metadata?: TickPublishMetadata): Promise<void>;
}

export interface MainOptions {
  mockMode: boolean;
  redisUrl?: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}

export interface MockTickOptions {
  llmClient: LlmClient;
  sink?: PublishSink | null;
  now?: () => number;
  model: string;
  worldModel?: WorldModel;
}

export interface MockTickSummary {
  totalCommands: number;
  totalNarrations: number;
  chatSignalCount: number;
  skipped: boolean;
  durationMs: number;
}

export function getMockCompletionMarker(): string {
  return MOCK_COMPLETION_MARKER;
}

export async function runMockTickForTest(options: MockTickOptions): Promise<MockTickSummary> {
  const startMs = Date.now();
  const { llmClient, model, sink } = options;
  const now = options.now ?? (() => Date.now());
  const worldModel = options.worldModel ?? new WorldModel();
  const state = createMockWorldState();

  const agents = createDefaultAgents(now);
  const result = await runTick(state, {
    agents,
    llmClient,
    model,
    worldModel,
    publishCommands: async (request) => {
      await sink?.publishCommands("merged", request.commands, request.metadata);
    },
    publishNarrations: async (request) => {
      await sink?.publishNarrations(request.narrations, request.metadata);
    },
    logger: console,
  });

  return {
    totalCommands: result.totalCommands,
    totalNarrations: result.totalNarrations,
    chatSignalCount: 0,
    skipped: result.skipped,
    durationMs: Date.now() - startMs,
  };
}

export async function main(options: MainOptions): Promise<void> {
  if (options.mockMode) {
    const llmClient = createMockClient();
    await runMockTickForTest({
      llmClient,
      model: options.model,
      sink: null,
    });
    console.log(MOCK_COMPLETION_MARKER);
    return;
  }

  const config = {
    mockMode: false,
    model: options.model,
    redisUrl: options.redisUrl ?? "redis://127.0.0.1:6379",
    baseUrl: options.baseUrl ?? null,
    apiKey: options.apiKey ?? null,
  };

  // 顿悟 runtime（事件驱动，独立于 tick loop，与 runRuntime 并行）。
  const insightCleanup = await startInsightRuntime({
    redisUrl: config.redisUrl,
    baseUrl: options.baseUrl,
    apiKey: options.apiKey,
    model: options.model,
  });
  const deathInsightCleanup = await startDeathInsightRuntime({
    redisUrl: config.redisUrl,
    baseUrl: options.baseUrl,
    apiKey: options.apiKey,
    model: options.model,
  });
  const skillLvUpCleanup = await startSkillLvUpRuntime({
    redisUrl: config.redisUrl,
    baseUrl: options.baseUrl,
    apiKey: options.apiKey,
    model: options.model,
  });
  const tribulationCleanup = await startTribulationRuntime({
    redisUrl: config.redisUrl,
    baseUrl: options.baseUrl,
    apiKey: options.apiKey,
    model: options.model,
  });
  const woliuCleanup = await startWoliuRuntime({
    redisUrl: config.redisUrl,
    baseUrl: options.baseUrl,
    apiKey: options.apiKey,
    model: options.model,
  });
  const zhenmaiCleanup = await startZhenmaiRuntime({
    redisUrl: config.redisUrl,
  });

  const heartDemonCleanup = await startHeartDemonRuntime({
    redisUrl: config.redisUrl,
    baseUrl: options.baseUrl,
    apiKey: options.apiKey,
    model: options.model,
  });

  try {
    await runRuntime(config);
  } finally {
    await heartDemonCleanup();
    await zhenmaiCleanup();
    await woliuCleanup();
    await tribulationCleanup();
    await skillLvUpCleanup();
    await deathInsightCleanup();
    await insightCleanup();
  }
}

async function startWoliuRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof WoliuNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof WoliuNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new WoliuNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] woliu runtime online"))
    .catch((error) => console.warn("[tiandao] woliu runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] woliu runtime disconnect error:", error);
    }
  };
}

async function startZhenmaiRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof ZhenmaiNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof ZhenmaiNarrationRuntime
  >[0]["pub"];

  const runtime = new ZhenmaiNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] zhenmai runtime online"))
    .catch((error) => console.warn("[tiandao] zhenmai runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] zhenmai runtime disconnect error:", error);
    }
  };
}

async function startHeartDemonRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof HeartDemonRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof HeartDemonRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new HeartDemonRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] heart demon runtime online"))
    .catch((error) => console.warn("[tiandao] heart demon runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] heart demon runtime disconnect error:", error);
    }
  };
}

async function startInsightRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof InsightRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof InsightRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new InsightRuntime({ llm, model: opts.model, sub, pub });
  // Fire-and-forget connect (don't block startup if Redis is unreachable;
  // ioredis retries internally, and the tick-based runtime has its own retry loop).
  runtime
    .connect()
    .then(() => console.log("[tiandao] insight runtime online"))
    .catch((error) => console.warn("[tiandao] insight runtime failed to start:", error));
  return async () => {
    // Best-effort; don't hang shutdown if Redis is unreachable.
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] insight runtime disconnect error:", error);
    }
  };
}

async function startTribulationRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TribulationNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TribulationNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new TribulationNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] tribulation runtime online"))
    .catch((error) => console.warn("[tiandao] tribulation runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] tribulation runtime disconnect error:", error);
    }
  };
}

async function startDeathInsightRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof DeathInsightRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof DeathInsightRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new DeathInsightRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] death insight runtime online"))
    .catch((error) => console.warn("[tiandao] death insight runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] death insight runtime disconnect error:", error);
    }
  };
}

async function startSkillLvUpRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof SkillLvUpNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof SkillLvUpNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new SkillLvUpNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] skill lv up runtime online"))
    .catch((error) => console.warn("[tiandao] skill lv up runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] skill lv up runtime disconnect error:", error);
    }
  };
}

// Auto-run only when executed directly as CLI entry point
const __filename = fileURLToPath(import.meta.url);
if (process.argv[1] === __filename) {
  loadEnv();
  const config = resolveRuntimeConfig(process.argv, process.env);
  main({
    mockMode: config.mockMode,
    redisUrl: config.redisUrl,
    baseUrl: config.baseUrl ?? undefined,
    apiKey: config.apiKey ?? undefined,
    model: config.model,
  }).catch((err) => {
    console.error("[tiandao] fatal:", err);
    process.exit(1);
  });
}
