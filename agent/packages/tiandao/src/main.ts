import { fileURLToPath } from "node:url";
import Redis from "ioredis";
import type { Command, Narration } from "@bong/schema";
import { CraftNarrationRuntime } from "./craft-runtime.js";
import { DeathInsightRuntime } from "./death-insight-runtime.js";
import { DuguNarrationRuntime } from "./dugu-narration.js";
import { DuguV2NarrationRuntime } from "./dugu_v2_runtime.js";
import { HeartDemonRuntime } from "./heart-demon-runtime.js";
import { InsightRuntime } from "./insight-runtime.js";
import { PoliticalNarrationRuntime } from "./political-narration.js";
import { ScatteredCultivatorNarrationRuntime } from "./scattered-cultivator-narration.js";
import { SkillLvUpNarrationRuntime } from "./skill-lv-up-runtime.js";
import { TribulationNarrationRuntime } from "./tribulation-runtime.js";
import { TuikeNarrationRuntime } from "./tuike-narration.js";
import { VoidActionNarrationRuntime } from "./void-actions-runtime.js";
import { WoliuNarrationRuntime } from "./woliu-narration.js";
import { YidaoNarrationRuntime } from "./yidao-runtime.js";
import { WoliuV2NarrationRuntime } from "./woliu_v2_runtime.js";
import { ZhenmaiNarrationRuntime } from "./zhenmai-narration.js";
import { ZhenfaV2NarrationRuntime } from "./zhenfa-v2-runtime.js";
import { AnqiNarrationRuntime } from "./anqi-narration.js";
import { BaomaiV3NarrationRuntime } from "./baomai-v3-runtime.js";
import { createClient as createLlmClient, createMockClient, type LlmClient } from "./llm.js";
import { createMockWorldState } from "./mock-state.js";
import {
  createDefaultAgents,
  loadEnv,
  resolveRuntimeConfig,
  runRuntime,
  runTick,
  type RuntimeConfig,
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
  auxiliaryRuntimeStarter?: AuxiliaryRuntimeStarter;
}

export type RuntimeCleanup = () => Promise<void>;
export type AuxiliaryRuntimeStarter = (config: RuntimeConfig) => Promise<RuntimeCleanup[]>;

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

  const cleanupFns = await (options.auxiliaryRuntimeStarter ?? startAuxiliaryRuntimes)(config);

  try {
    await runRuntime(config);
  } finally {
    for (const cleanup of cleanupFns) {
      await cleanup();
    }
  }
}

async function startAuxiliaryRuntimes(config: RuntimeConfig): Promise<RuntimeCleanup[]> {
  const runtimeOpts = {
    redisUrl: config.redisUrl,
    baseUrl: config.baseUrl ?? undefined,
    apiKey: config.apiKey ?? undefined,
    model: config.model,
  };

  // 顿悟 runtime（事件驱动，独立于 tick loop，与 runRuntime 并行）。
  const insightCleanup = await startInsightRuntime({
    ...runtimeOpts,
  });
  const deathInsightCleanup = await startDeathInsightRuntime({
    ...runtimeOpts,
  });
  const skillLvUpCleanup = await startSkillLvUpRuntime({
    ...runtimeOpts,
  });
  const tribulationCleanup = await startTribulationRuntime({
    ...runtimeOpts,
  });
  const voidActionCleanup = await startVoidActionRuntime({
    ...runtimeOpts,
  });
  const woliuCleanup = await startWoliuRuntime({
    ...runtimeOpts,
  });
  const woliuV2Cleanup = await startWoliuV2Runtime({
    ...runtimeOpts,
  });
  const zhenmaiCleanup = await startZhenmaiRuntime({
    redisUrl: config.redisUrl,
  });
  const zhenfaV2Cleanup = await startZhenfaV2Runtime({
    redisUrl: config.redisUrl,
  });
  const yidaoCleanup = await startYidaoRuntime({
    redisUrl: config.redisUrl,
  });
  const baomaiV3Cleanup = await startBaomaiV3Runtime({
    redisUrl: config.redisUrl,
  });
  const anqiCleanup = await startAnqiRuntime({
    ...runtimeOpts,
  });
  const tuikeCleanup = await startTuikeRuntime({
    ...runtimeOpts,
  });
  const duguCleanup = await startDuguRuntime({
    ...runtimeOpts,
  });
  const duguV2Cleanup = await startDuguV2Runtime({
    ...runtimeOpts,
  });
  const scatteredCultivatorCleanup = await startScatteredCultivatorRuntime({
    redisUrl: config.redisUrl,
  });

  const heartDemonCleanup = await startHeartDemonRuntime({
    ...runtimeOpts,
  });
  const craftCleanup = await startCraftRuntime({
    ...runtimeOpts,
  });
  const politicalCleanup = await startPoliticalRuntime({
    ...runtimeOpts,
  });

  return [
    politicalCleanup,
    heartDemonCleanup,
    craftCleanup,
    anqiCleanup,
    baomaiV3Cleanup,
    yidaoCleanup,
    zhenmaiCleanup,
    zhenfaV2Cleanup,
    tuikeCleanup,
    duguV2Cleanup,
    duguCleanup,
    scatteredCultivatorCleanup,
    woliuV2Cleanup,
    woliuCleanup,
    voidActionCleanup,
    tribulationCleanup,
    skillLvUpCleanup,
    deathInsightCleanup,
    insightCleanup,
  ];
}

async function startWoliuV2Runtime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof WoliuV2NarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof WoliuV2NarrationRuntime
  >[0]["pub"];
  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({ baseURL: opts.baseUrl, apiKey: opts.apiKey, model: opts.model })
    : createMockClient();
  const runtime = new WoliuV2NarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] woliu v2 runtime online"))
    .catch((error) => console.warn("[tiandao] woliu v2 runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] woliu v2 runtime disconnect error:", error);
    }
  };
}

async function startPoliticalRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof PoliticalNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof PoliticalNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new PoliticalNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] political runtime online"))
    .catch((error) => console.warn("[tiandao] political runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] political runtime disconnect error:", error);
    }
  };
}

async function startYidaoRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof YidaoNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof YidaoNarrationRuntime
  >[0]["pub"];

  const runtime = new YidaoNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] yidao runtime online"))
    .catch((error) => console.warn("[tiandao] yidao runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] yidao runtime disconnect error:", error);
    }
  };
}

async function startBaomaiV3Runtime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof BaomaiV3NarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof BaomaiV3NarrationRuntime
  >[0]["pub"];

  const runtime = new BaomaiV3NarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] baomai v3 runtime online"))
    .catch((error) => console.warn("[tiandao] baomai v3 runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] baomai v3 runtime disconnect error:", error);
    }
  };
}

async function startZhenfaV2Runtime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof ZhenfaV2NarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof ZhenfaV2NarrationRuntime
  >[0]["pub"];

  const runtime = new ZhenfaV2NarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] zhenfa v2 runtime online"))
    .catch((error) => console.warn("[tiandao] zhenfa v2 runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] zhenfa v2 runtime disconnect error:", error);
    }
  };
}

async function startScatteredCultivatorRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof ScatteredCultivatorNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof ScatteredCultivatorNarrationRuntime
  >[0]["pub"];

  const runtime = new ScatteredCultivatorNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] scattered cultivator runtime online"))
    .catch((error) =>
      console.warn("[tiandao] scattered cultivator runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] scattered cultivator runtime disconnect error:", error);
    }
  };
}

async function startAnqiRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof AnqiNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof AnqiNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new AnqiNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] anqi runtime online"))
    .catch((error) => console.warn("[tiandao] anqi runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] anqi runtime disconnect error:", error);
    }
  };
}

async function startTuikeRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TuikeNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TuikeNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new TuikeNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] tuike runtime online"))
    .catch((error) => console.warn("[tiandao] tuike runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] tuike runtime disconnect error:", error);
    }
  };
}

async function startDuguRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof DuguNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof DuguNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new DuguNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] dugu runtime online"))
    .catch((error) => console.warn("[tiandao] dugu runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] dugu runtime disconnect error:", error);
    }
  };
}

async function startDuguV2Runtime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof DuguV2NarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof DuguV2NarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new DuguV2NarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] dugu v2 runtime online"))
    .catch((error) => console.warn("[tiandao] dugu v2 runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] dugu v2 runtime disconnect error:", error);
    }
  };
}

async function startCraftRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof CraftNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof CraftNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new CraftNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] craft runtime online"))
    .catch((error) => console.warn("[tiandao] craft runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] craft runtime disconnect error:", error);
    }
  };
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

async function startVoidActionRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof VoidActionNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof VoidActionNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new VoidActionNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] void action runtime online"))
    .catch((error) => console.warn("[tiandao] void action runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] void action runtime disconnect error:", error);
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
