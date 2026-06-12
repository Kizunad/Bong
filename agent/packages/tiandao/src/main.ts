import { fileURLToPath } from "node:url";
import Redis from "ioredis";
import type { Command, Narration } from "@bong/schema";
import { CraftNarrationRuntime } from "./craft-runtime.js";
import { DeathInsightRuntime } from "./death-insight-runtime.js";
import { DuguNarrationRuntime } from "./dugu-narration.js";
import { DuguV2NarrationRuntime } from "./dugu_v2_runtime.js";
import { HeartDemonRuntime } from "./heart-demon-runtime.js";
import { InsightRuntime } from "./insight-runtime.js";
import { OffscreenWarNarrationRuntime } from "./offscreen-war-narration.js";
import { WarOutcomeNarrationRuntime } from "./war-outcome-narration.js";
import { PoliticalNarrationRuntime } from "./political-narration.js";
import { PoisonTraitNarrationRuntime } from "./poison-trait-runtime.js";
import { ElderEncounterNarrationRuntime } from "./elder-encounter-narration.js";
import { ScatteredCultivatorNarrationRuntime } from "./scattered-cultivator-narration.js";
import { SkillLvUpNarrationRuntime } from "./skill-lv-up-runtime.js";
import { SpiritTreasureDialogueRuntime } from "./spirit-treasure-dialogue-runtime.js";
import { TribulationNarrationRuntime } from "./tribulation-runtime.js";
import { TiandaoHuntNarrationRuntime } from "./tiandao-hunt-narration-runtime.js";
import { TuikeNarrationRuntime } from "./tuike-narration.js";
import { TuikeAshDecayNarrationRuntime } from "./tuike_ash_runtime.js";
import { TuikeV2NarrationRuntime } from "./tuike_v2_runtime.js";
import { VoidActionNarrationRuntime } from "./void-actions-runtime.js";
import { WoliuNarrationRuntime } from "./woliu-narration.js";
import { YidaoNarrationRuntime } from "./yidao-runtime.js";
import { WoliuV2NarrationRuntime } from "./woliu_v2_runtime.js";
import { ZhenmaiNarrationRuntime } from "./zhenmai-narration.js";
import { ZhenfaV2NarrationRuntime } from "./zhenfa-v2-runtime.js";
import { AnqiNarrationRuntime } from "./anqi-narration.js";
// plan-agent-ui-data-v1 P2：天道 UI runtime（UiRenderer + UiResponseConsumer 接线）
import { AgentUiRuntime } from "./ui/agentUiRuntime.js";
import { BaomaiV3NarrationRuntime } from "./baomai-v3-runtime.js";
import { BaomaiV4NarrationRuntime } from "./baomai-v4-runtime.js";
import { MeridianSeveredNarrationRuntime } from "./meridian-severed-runtime.js";
import { VoidErosionNarrationRuntime } from "./void_erosion_runtime.js";
import { MutationNarrationRuntime } from "./mutation-narration-runtime.js";
import { BreakthroughCinematicNarrationRuntime } from "./breakthrough-cinematic-narration.js";
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
  const breakthroughCinematicCleanup = await startBreakthroughCinematicRuntime({
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
  // plan-combat-skill-feedback-bridges-v1 P0: 经脉断脉叙事 runtime（订阅 bong:meridian_severed）。
  const meridianSeveredCleanup = await startMeridianSeveredNarrationRuntimeInternal({
    redisUrl: config.redisUrl,
  });
  // plan-combat-skill-feedback-bridges-v1 P3: 我流虚蚀阶段推进叙事 runtime（订阅 bong:void_erosion_event）。
  const voidErosionCleanup = await startVoidErosionRuntime({
    ...runtimeOpts,
  });
  // plan-combat-skill-feedback-bridges-v1 P1: baomai_v4 叙事 runtime（psubscribe bong:baomai_v4/*）。
  const baomaiV4Cleanup = await startBaomaiV4Runtime({
    redisUrl: config.redisUrl,
  });
  // plan-dandao-runtime-wiring-v1 P2: 变异叙事 runtime（订阅 bong:mutation_event，stage 3+ 出 narration）。
  const mutationNarrationCleanup = await startMutationNarrationRuntime({
    redisUrl: config.redisUrl,
  });
  const anqiCleanup = await startAnqiRuntime({
    ...runtimeOpts,
  });
  const tuikeCleanup = await startTuikeRuntime({
    ...runtimeOpts,
  });
  const tuikeV2Cleanup = await startTuikeV2Runtime({
    ...runtimeOpts,
  });
  // plan-combat-skill-feedback-bridges-v1 P6: 蜕壳灰烬入包叙事 runtime（订阅 bong:tuike_v2/ash_decay）。
  const tuikeAshCleanup = await startTuikeAshRuntime({
    redisUrl: config.redisUrl,
  });
  const duguCleanup = await startDuguRuntime({
    ...runtimeOpts,
  });
  const duguV2Cleanup = await startDuguV2Runtime({
    ...runtimeOpts,
  });
  const poisonTraitCleanup = await startPoisonTraitRuntime({
    redisUrl: config.redisUrl,
  });
  // plan-dying-elder-v1 P3: 垂死大能遭遇叙事 runtime（订阅 bong:elder_encounter）。
  const elderEncounterCleanup = await startElderEncounterRuntime({
    redisUrl: config.redisUrl,
  });
  const scatteredCultivatorCleanup = await startScatteredCultivatorRuntime({
    redisUrl: config.redisUrl,
  });
  // plan-offscreen-war-v1 P4：离屏派系消长叙事（订阅 bong:npc/death，聚合 dormant 互殴
  // 战死 → emit broadcast 散修消长 narration）。事件驱动，独立于 tick loop。
  const offscreenWarCleanup = await startOffscreenWarRuntime({
    redisUrl: config.redisUrl,
  });
  // plan-offscreen-war-v1 P9：战事结算匿名叙事（订阅 bong:faction/war，settling 阶段播报）。
  const warOutcomeCleanup = await startWarOutcomeNarrationRuntime({
    redisUrl: config.redisUrl,
  });

  const heartDemonCleanup = await startHeartDemonRuntime({
    ...runtimeOpts,
  });
  const craftCleanup = await startCraftRuntime({
    ...runtimeOpts,
  });
  const spiritTreasureCleanup = await startSpiritTreasureRuntime({
    ...runtimeOpts,
    model: process.env.SPIRIT_TREASURE_MODEL ?? "claude-haiku-4-5-20251001",
  });
  const politicalCleanup = await startPoliticalRuntime({
    ...runtimeOpts,
  });
  const tiandaoHuntCleanup = await startTiandaoHuntNarrationRuntime({
    ...runtimeOpts,
  });
  // plan-agent-ui-data-v1 P2：天道 UI 响应消费 runtime（订阅 bong:agent_ui_response）。
  const agentUiResponseCleanup = await startAgentUiResponseRuntime({
    redisUrl: config.redisUrl,
  });

  return [
    agentUiResponseCleanup,
    tiandaoHuntCleanup,
    politicalCleanup,
    spiritTreasureCleanup,
    heartDemonCleanup,
    craftCleanup,
    anqiCleanup,
    mutationNarrationCleanup,
    meridianSeveredCleanup,
    voidErosionCleanup,
    baomaiV4Cleanup,
    baomaiV3Cleanup,
    yidaoCleanup,
    zhenmaiCleanup,
    zhenfaV2Cleanup,
    tuikeV2Cleanup,
    tuikeAshCleanup,
    tuikeCleanup,
    duguV2Cleanup,
    duguCleanup,
    poisonTraitCleanup,
    elderEncounterCleanup,
    scatteredCultivatorCleanup,
    offscreenWarCleanup,
    warOutcomeCleanup,
    woliuV2Cleanup,
    woliuCleanup,
    voidActionCleanup,
    tribulationCleanup,
    breakthroughCinematicCleanup,
    skillLvUpCleanup,
    deathInsightCleanup,
    insightCleanup,
  ];
}

async function startTiandaoHuntNarrationRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TiandaoHuntNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TiandaoHuntNarrationRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new TiandaoHuntNarrationRuntime({
    llm,
    model: opts.model,
    sub,
    pub,
  });
  runtime
    .connect()
    .then(() => console.log("[tiandao] tiandao hunt narration runtime online"))
    .catch((error) =>
      console.warn("[tiandao] tiandao hunt narration runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] tiandao hunt narration runtime disconnect error:", error);
    }
  };
}

async function startBreakthroughCinematicRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof BreakthroughCinematicNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof BreakthroughCinematicNarrationRuntime
  >[0]["pub"];
  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({ baseURL: opts.baseUrl, apiKey: opts.apiKey, model: opts.model })
    : createMockClient();
  const runtime = new BreakthroughCinematicNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] breakthrough cinematic runtime online"))
    .catch((error) =>
      console.warn("[tiandao] breakthrough cinematic runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] breakthrough cinematic runtime disconnect error:", error);
    }
  };
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

async function startSpiritTreasureRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof SpiritTreasureDialogueRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof SpiritTreasureDialogueRuntime
  >[0]["pub"];

  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({
        baseURL: opts.baseUrl,
        apiKey: opts.apiKey,
        model: opts.model,
      })
    : createMockClient();

  const runtime = new SpiritTreasureDialogueRuntime({
    llm,
    model: opts.model,
    sub,
    pub,
  });
  runtime
    .connect()
    .then(() => console.log("[tiandao] spirit treasure runtime online"))
    .catch((error) =>
      console.warn("[tiandao] spirit treasure runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] spirit treasure runtime disconnect error:", error);
    }
  };
}

async function startTuikeV2Runtime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TuikeV2NarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TuikeV2NarrationRuntime
  >[0]["pub"];
  const llm: LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({ baseURL: opts.baseUrl, apiKey: opts.apiKey, model: opts.model })
    : createMockClient();
  const runtime = new TuikeV2NarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] tuike v2 runtime online"))
    .catch((error) => console.warn("[tiandao] tuike v2 runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] tuike v2 runtime disconnect error:", error);
    }
  };
}

async function startTuikeAshRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TuikeAshDecayNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof TuikeAshDecayNarrationRuntime
  >[0]["pub"];
  const runtime = new TuikeAshDecayNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] tuike ash decay runtime online"))
    .catch((error) =>
      console.warn("[tiandao] tuike ash decay runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] tuike ash decay runtime disconnect error:", error);
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

async function startMutationNarrationRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof MutationNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof MutationNarrationRuntime
  >[0]["pub"];

  const runtime = new MutationNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] mutation narration runtime online"))
    .catch((error) => console.warn("[tiandao] mutation narration runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] mutation narration runtime disconnect error:", error);
    }
  };
}

async function startMeridianSeveredNarrationRuntimeInternal(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof MeridianSeveredNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof MeridianSeveredNarrationRuntime
  >[0]["pub"];

  const runtime = new MeridianSeveredNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() =>
      console.log("[tiandao] meridian-severed narration runtime online"),
    )
    .catch((error) =>
      console.warn(
        "[tiandao] meridian-severed narration runtime failed to start:",
        error,
      ),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn(
        "[tiandao] meridian-severed narration runtime disconnect error:",
        error,
      );
    }
  };
}

async function startBaomaiV4Runtime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof BaomaiV4NarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof BaomaiV4NarrationRuntime
  >[0]["pub"];

  const runtime = new BaomaiV4NarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] baomai-v4 narration runtime online"))
    .catch((error) =>
      console.warn("[tiandao] baomai-v4 narration runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] baomai-v4 narration runtime disconnect error:", error);
    }
  };
}

// plan-combat-skill-feedback-bridges-v1 P3 — 我流虚蚀阶段推进叙事 runtime 启动函数
async function startVoidErosionRuntime(opts: {
  redisUrl: string;
  baseUrl?: string;
  apiKey?: string;
  model: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof VoidErosionNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof VoidErosionNarrationRuntime
  >[0]["pub"];
  const llm: import("./llm.js").LlmClient = opts.baseUrl && opts.apiKey
    ? createLlmClient({ baseURL: opts.baseUrl, apiKey: opts.apiKey, model: opts.model })
    : createMockClient();
  const runtime = new VoidErosionNarrationRuntime({ llm, model: opts.model, sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] void-erosion narration runtime online"))
    .catch((error) =>
      console.warn("[tiandao] void-erosion narration runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] void-erosion narration runtime disconnect error:", error);
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

// plan-dying-elder-v1 P3
async function startElderEncounterRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof ElderEncounterNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof ElderEncounterNarrationRuntime
  >[0]["pub"];

  const runtime = new ElderEncounterNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] elder encounter narration runtime online"))
    .catch((error) =>
      console.warn("[tiandao] elder encounter runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] elder encounter runtime disconnect error:", error);
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

async function startOffscreenWarRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof OffscreenWarNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof OffscreenWarNarrationRuntime
  >[0]["pub"];

  const runtime = new OffscreenWarNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] offscreen war runtime online"))
    .catch((error) =>
      console.warn("[tiandao] offscreen war runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] offscreen war runtime disconnect error:", error);
    }
  };
}

async function startPoisonTraitRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof PoisonTraitNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof PoisonTraitNarrationRuntime
  >[0]["pub"];

  const runtime = new PoisonTraitNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] poison trait runtime online"))
    .catch((error) => console.warn("[tiandao] poison trait runtime failed to start:", error));
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] poison trait runtime disconnect error:", error);
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

async function startWarOutcomeNarrationRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof WarOutcomeNarrationRuntime
  >[0]["sub"];
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof WarOutcomeNarrationRuntime
  >[0]["pub"];

  const runtime = new WarOutcomeNarrationRuntime({ sub, pub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] war outcome narration runtime online"))
    .catch((error) =>
      console.warn("[tiandao] war outcome narration runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] war outcome narration runtime disconnect error:", error);
    }
  };
}

// Auto-run only when executed directly as CLI entry point
const __filename = fileURLToPath(import.meta.url);
// plan-agent-ui-data-v1 P2：启动天道 UI runtime（UiRenderer + UiResponseConsumer 接线）。
// onButtonClick 回调把 button_click 追加到 pendingButtonClicks 队列供下一轮推演 drain。
// onSessionEnd 回调记录 session 结束事件（dismissed / timeout）。
async function startAgentUiResponseRuntime(opts: {
  redisUrl: string;
}): Promise<() => Promise<void>> {
  const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
    Redis) as new (url: string) => unknown;
  const pub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof AgentUiRuntime
  >[0]["pub"];
  const sub = new IORedisCtor(opts.redisUrl) as ConstructorParameters<
    typeof AgentUiRuntime
  >[0]["sub"];

  const runtime = new AgentUiRuntime({ pub, sub });
  runtime
    .connect()
    .then(() => console.log("[tiandao] agent ui runtime online (renderer + response consumer)"))
    .catch((error) =>
      console.warn("[tiandao] agent ui runtime failed to start:", error),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn("[tiandao] agent ui runtime disconnect error:", error);
    }
  };
}

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
