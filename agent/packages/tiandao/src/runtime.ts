import type {
  AgentWorldModelEnvelopeV1,
  ChatMessageV1,
  ChatSignal,
  Command,
  Narration,
  WorldStateV1,
} from "@bong/schema";
import dotenv from "dotenv";
import { mkdir, readdir, unlink, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, URL } from "node:url";
import { Arbiter } from "./arbiter.js";
import { TiandaoAgent, resolveAgentTools } from "./agent.js";
import type { AgentDecisionWithMetadata } from "./agent.js";
import { mergeChatSignals, processChatBatch } from "./chat-processor.js";
import { CALAMITY_RECIPE, MUTATION_RECIPE, ERA_RECIPE } from "./context.js";
import { createClient, createMockClient, LlmBackoffError, LlmTimeoutError, type LlmClient } from "./llm.js";
import { createMockWorldState } from "./mock-state.js";
import {
  NARRATION_LOW_SCORE_THRESHOLD,
  evaluateNarrations,
  formatNarrationLowScoreWarning,
  summarizeNarrationAverage,
} from "./narration-eval.js";
import type { AgentDecision } from "./parse.js";
import { RedisIpc } from "./redis-ipc.js";
import {
  emptyErrorBreakdown,
  JsonLogSink,
  NoopTelemetrySink,
  RollingSummarySink,
  type TelemetrySink,
  type TickAgentResult,
  type TickErrorBreakdown,
  type TickMetrics,
} from "./telemetry.js";
import { WorldModel, type WorldModelSnapshot } from "./world-model.js";

declare global {
  interface NumberConstructor {
    isFinite(number: unknown): number is number;
  }
}

export const MOCK_FLAG = "--mock";
export const DEFAULT_MODEL = "gpt-5.4-mini";
export const DEFAULT_REDIS_URL = "redis://127.0.0.1:6379";
const TICK_INTERVAL_MS = 5_000;
const CHAT_DRAIN_WINDOW = 128;
const LOOP_BACKOFF_BASE_MS = 1_000;
const LOOP_BACKOFF_MAX_MS = 30_000;
const SNAPSHOT_INTERVAL_TICKS = 100;
const SNAPSHOT_KEEP_COUNT = 5;
const SNAPSHOT_FILE_PREFIX = "tiandao-snapshot-";
const SNAPSHOT_FILE_SUFFIX = ".json";
export const ALLOWED_LLM_MODELS = Object.freeze([DEFAULT_MODEL, "gpt-5.4"] as const);
export const MODEL_ROUTE_ROLES = Object.freeze([
  "default",
  "annotate",
  "calamity",
  "mutation",
  "era",
] as const);

export type RuntimeModelRole = (typeof MODEL_ROUTE_ROLES)[number];
export type TickAgentRole = Extract<RuntimeModelRole, "default" | "calamity" | "mutation" | "era">;

export interface RuntimeModelOverrides {
  default: string;
  annotate: string;
  calamity: string;
  mutation: string;
  era: string;
}

export interface RuntimeRoleClients {
  default: LlmClient;
  annotate: LlmClient;
  calamity: LlmClient;
  mutation: LlmClient;
  era: LlmClient;
}

const __dirname = dirname(fileURLToPath(import.meta.url));

export interface RuntimeConfig {
  mockMode: boolean;
  model: string;
  modelOverrides?: RuntimeModelOverrides;
  redisUrl: string;
  baseUrl: string | null;
  apiKey: string | null;
}

export interface TickAgent {
  name: string;
  tick(client: LlmClient, model: string, state: WorldStateV1): Promise<AgentDecision | null>;
  setChatSignals?(signals: ChatSignal[]): void;
  setWorldModel?(worldModel: WorldModel): void;
}

export interface RuntimeRedis {
  connect(): Promise<void>;
  getLatestState(): WorldStateV1 | null;
  drainPlayerChat(options?: { maxItems?: number; logger?: Pick<typeof console, "warn"> }): Promise<ChatMessageV1[]>;
  publishCommands(request: CommandPublishRequest): Promise<void>;
  publishNarrations(request: NarrationPublishRequest): Promise<void>;
  publishAgentWorldModel?(request: {
    source: NonNullable<AgentWorldModelEnvelopeV1["source"]>;
    snapshot: AgentWorldModelEnvelopeV1["snapshot"];
    metadata: TickPublishMetadata;
  }): Promise<void>;
  disconnect(): Promise<void>;
}

export interface RuntimeDeps {
  agents?: TickAgent[];
  createRedis?: (url: string) => RuntimeRedis;
  createClient?: (args: { baseURL: string; apiKey: string; model: string }) => LlmClient;
  createMockClient?: () => LlmClient;
  loadMockState?: () => WorldStateV1;
  worldModel?: WorldModel;
  sleep?: (ms: number) => Promise<void>;
  logger?: Pick<typeof console, "log" | "error" | "warn">;
  maxLoopIterations?: number;
  telemetrySink?: TelemetrySink;
}

interface WorldStateCursor {
  tick: number;
  ts: number | null;
}

export interface TickDeps {
  agents: TickAgent[];
  llmClient: LlmClient;
  model: string;
  llmClientsByRole?: RuntimeRoleClients;
  modelOverrides?: RuntimeModelOverrides;
  chatSignals?: ChatSignal[];
  worldModel?: WorldModel;
  publishCommands: (request: CommandPublishRequest) => Promise<void>;
  publishNarrations: (request: NarrationPublishRequest) => Promise<void>;
  logger: Pick<typeof console, "log" | "error">;
  tickStartedAtMs?: number;
  reconnectCount?: number;
  backoffCount?: number;
  staleStateSkipped?: boolean;
  chatSignalCount?: number;
  telemetrySink?: TelemetrySink;
  telemetryWarnLogger?: Pick<typeof console, "warn">;
}

export interface TickPublishMetadata {
  sourceTick: number;
  correlationId: string;
}

export interface CommandPublishRequest {
  source: "arbiter";
  commands: Command[];
  metadata: TickPublishMetadata;
}

export interface NarrationPublishRequest {
  narrations: Narration[];
  metadata: TickPublishMetadata;
}

export interface TickResult {
  totalCommands: number;
  totalNarrations: number;
  skipped: boolean;
  metadata: TickPublishMetadata;
  metrics: TickMetrics;
}

export function loadEnv(): void {
  dotenv.config({ path: resolve(__dirname, "../../../../.env") });
}

export function resolveRuntimeConfig(
  argv: string[] = process.argv,
  env: NodeJS.ProcessEnv = process.env,
): RuntimeConfig {
  const mockMode = argv.includes(MOCK_FLAG);
  const defaultModel = resolveAllowedModelOverride("default", env.LLM_MODEL ?? DEFAULT_MODEL);
  return {
    mockMode,
    model: defaultModel,
    modelOverrides: Object.freeze({
      default: defaultModel,
      annotate: resolveAllowedModelOverride("annotate", env.LLM_MODEL_ANNOTATE ?? defaultModel),
      calamity: resolveAllowedModelOverride("calamity", env.LLM_MODEL_CALAMITY ?? defaultModel),
      mutation: resolveAllowedModelOverride("mutation", env.LLM_MODEL_MUTATION ?? defaultModel),
      era: resolveAllowedModelOverride("era", env.LLM_MODEL_ERA ?? defaultModel),
    }),
    redisUrl: env.REDIS_URL ?? DEFAULT_REDIS_URL,
    baseUrl: env.LLM_BASE_URL ?? null,
    apiKey: env.LLM_API_KEY ?? null,
  };
}

export function createDefaultAgents(
  optionsOrNow:
    | { now?: () => number; modelOverrides?: RuntimeModelOverrides }
    | (() => number)
    | undefined = {},
): TiandaoAgent[] {
  const normalizedOptions =
    typeof optionsOrNow === "function"
      ? { now: optionsOrNow, modelOverrides: undefined }
      : (optionsOrNow ?? {});
  const { now, modelOverrides } = normalizedOptions;
  return [
    new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 180_000,
      model: modelOverrides?.calamity,
      tools: resolveAgentTools("calamity.md"),
      now,
    }),
    new TiandaoAgent({
      name: "mutation",
      skillFile: "mutation.md",
      recipe: MUTATION_RECIPE,
      intervalMs: 600_000,
      model: modelOverrides?.mutation,
      tools: resolveAgentTools("mutation.md"),
      now,
    }),
    new TiandaoAgent({
      name: "era",
      skillFile: "era.md",
      recipe: ERA_RECIPE,
      intervalMs: 36_000_000,
      model: modelOverrides?.era,
      tools: resolveAgentTools("era.md"),
      now,
    }),
  ];
}

function requireLlmCredentials(config: RuntimeConfig): { baseUrl: string; apiKey: string } {
  if (!config.baseUrl || !config.apiKey) {
    throw new Error("Missing LLM_BASE_URL or LLM_API_KEY in .env");
  }

  return {
    baseUrl: config.baseUrl,
    apiKey: config.apiKey,
  };
}

function resolveAllowedModelOverride(role: RuntimeModelRole, candidate: string): string {
  if (ALLOWED_LLM_MODELS.includes(candidate as (typeof ALLOWED_LLM_MODELS)[number])) {
    return candidate;
  }

  throw new Error(
    `[tiandao] invalid model override for role '${role}': '${candidate}'. Allowed values: ${ALLOWED_LLM_MODELS.join(
      ", ",
    )}`,
  );
}

function resolveModelOverrides(config: RuntimeConfig): RuntimeModelOverrides {
  if (config.modelOverrides) {
    return Object.freeze({
      default: resolveAllowedModelOverride("default", config.modelOverrides.default),
      annotate: resolveAllowedModelOverride("annotate", config.modelOverrides.annotate),
      calamity: resolveAllowedModelOverride("calamity", config.modelOverrides.calamity),
      mutation: resolveAllowedModelOverride("mutation", config.modelOverrides.mutation),
      era: resolveAllowedModelOverride("era", config.modelOverrides.era),
    });
  }

  const defaultModel = resolveAllowedModelOverride("default", config.model);
  return Object.freeze({
    default: defaultModel,
    annotate: defaultModel,
    calamity: defaultModel,
    mutation: defaultModel,
    era: defaultModel,
  });
}

function redactRedisUrlForLog(redisUrl: string): string {
  try {
    const parsed = new URL(redisUrl);
    return parsed.host || "[redacted redis endpoint]";
  } catch {
    return "[redacted redis endpoint]";
  }
}

export function createRuntimeClient(
  config: RuntimeConfig,
  deps: Pick<RuntimeDeps, "createClient" | "createMockClient"> = {},
): LlmClient {
  const modelOverrides = resolveModelOverrides(config);

  if (config.mockMode) {
    return (deps.createMockClient ?? createMockClient)();
  }

  const { baseUrl, apiKey } = requireLlmCredentials(config);
  const buildClient = deps.createClient ?? createClient;
  return buildClient({
    baseURL: baseUrl,
    apiKey,
    model: modelOverrides.default,
  });
}

export function createRuntimeClients(
  config: RuntimeConfig,
  deps: Pick<RuntimeDeps, "createClient" | "createMockClient"> = {},
): RuntimeRoleClients {
  const modelOverrides = resolveModelOverrides(config);

  if (config.mockMode) {
    const buildMockClient = deps.createMockClient ?? createMockClient;
    return {
      default: buildMockClient(),
      annotate: buildMockClient(),
      calamity: buildMockClient(),
      mutation: buildMockClient(),
      era: buildMockClient(),
    };
  }

  const { baseUrl, apiKey } = requireLlmCredentials(config);
  const buildClient = deps.createClient ?? createClient;

  return {
    default: buildClient({
      baseURL: baseUrl,
      apiKey,
      model: modelOverrides.default,
    }),
    annotate: buildClient({
      baseURL: baseUrl,
      apiKey,
      model: modelOverrides.annotate,
    }),
    calamity: buildClient({
      baseURL: baseUrl,
      apiKey,
      model: modelOverrides.calamity,
    }),
    mutation: buildClient({
      baseURL: baseUrl,
      apiKey,
      model: modelOverrides.mutation,
    }),
    era: buildClient({
      baseURL: baseUrl,
      apiKey,
      model: modelOverrides.era,
    }),
  };
}

export async function runTick(state: WorldStateV1, deps: TickDeps): Promise<TickResult> {
  const {
    agents,
    llmClient,
    model,
    llmClientsByRole,
    modelOverrides,
    chatSignals,
    worldModel,
    publishCommands,
    publishNarrations,
    logger,
    tickStartedAtMs,
    reconnectCount,
    backoffCount,
    staleStateSkipped,
    chatSignalCount,
    telemetrySink,
    telemetryWarnLogger,
  } = deps;
  const measuredTickStartMs = tickStartedAtMs ?? Date.now();
  const effectiveModelOverrides = modelOverrides ?? {
    default: model,
    annotate: model,
    calamity: model,
    mutation: model,
    era: model,
  };
  const effectiveRoleClients = llmClientsByRole ?? {
    default: llmClient,
    annotate: llmClient,
    calamity: llmClient,
    mutation: llmClient,
    era: llmClient,
  };
  const metadata: TickPublishMetadata = {
    sourceTick: state.tick,
    correlationId: `tiandao-tick-${state.tick}`,
  };

  worldModel?.updateState(state);
  applyWorldModelToAgents(agents, worldModel);
  applyChatSignalsToAgents(agents, chatSignals ?? []);
  logger.log("[tiandao] === tick start ===");
  logger.log(
    `[tiandao] tick: ${state.tick}, players: ${state.players.length}, zones: ${state.zones.length}, correlation_id: ${metadata.correlationId}`,
  );

  const results = await Promise.allSettled(
    agents.map((agent) => {
      const startedAtMs = Date.now();
      const agentRole = resolveTickAgentRole(agent.name);
      const agentClient = effectiveRoleClients[agentRole];
      const routedModel = effectiveModelOverrides[agentRole];
      return agent
        .tick(agentClient, routedModel, state)
        .then((decision) => ({
          decision,
          startedAtMs,
          endedAtMs: Date.now(),
        }))
        .catch((error) => {
          throw new AgentTickExecutionError(error, startedAtMs, Date.now());
        });
    }),
  );

  const sourcedDecisions: Array<{ source: string; decision: AgentDecision }> = [];
  const agentResults: TickAgentResult[] = [];
  let parseFailCount = 0;

  for (let i = 0; i < results.length; i++) {
    const result = results[i];
    const agent = agents[i];
    if (result.status === "fulfilled" && result.value.decision) {
      const agentDurationMs = Math.max(0, result.value.endedAtMs - result.value.startedAtMs);
      const decision = result.value.decision as AgentDecisionWithMetadata;
      const metadata = decision.__agentTickMetadata;
      const parseFailures = result.value.decision.parseFailures?.total ?? 0;
      parseFailCount += parseFailures;
      logger.log(
        `[tiandao][${agent.name}] commands: ${result.value.decision.commands.length}, narrations: ${result.value.decision.narrations.length}`,
      );
      logger.log(`[tiandao][${agent.name}] reasoning: ${result.value.decision.reasoning}`);
      sourcedDecisions.push({ source: agent.name, decision: result.value.decision });
      agentResults.push({
        name: agent.name,
        status: "ok",
        durationMs: Math.max(agentDurationMs, metadata?.durationMs ?? 0),
        commandCount: result.value.decision.commands.length,
        narrationCount: result.value.decision.narrations.length,
        tokensEstimated: metadata?.tokensEstimated ?? 0,
        model: metadata?.model ?? effectiveModelOverrides[resolveTickAgentRole(agent.name)],
      });
    } else if (result.status === "rejected") {
      const wrappedReason = toAgentTickError(result.reason);
      const agentDurationMs = Math.max(0, wrappedReason.endedAtMs - wrappedReason.startedAtMs);
      logger.error(`[tiandao][${agent.name}] error:`, wrappedReason.error);
      agentResults.push({
        name: agent.name,
        status: "error",
        durationMs: agentDurationMs,
        commandCount: 0,
        narrationCount: 0,
        tokensEstimated: 0,
        model: effectiveModelOverrides[resolveTickAgentRole(agent.name)],
      });
    } else {
      const agentDurationMs = Math.max(0, result.value.endedAtMs - result.value.startedAtMs);
      logger.log(`[tiandao][${agent.name}] skipped (not due yet)`);
      agentResults.push({
        name: agent.name,
        status: "skipped",
        durationMs: agentDurationMs,
        commandCount: 0,
        narrationCount: 0,
        tokensEstimated: 0,
        model: effectiveModelOverrides[resolveTickAgentRole(agent.name)],
      });
    }
  }

  const merged = new Arbiter(state).merge(sourcedDecisions);
  const totalCommands = sourcedDecisions.reduce((sum, { decision }) => sum + decision.commands.length, 0);
  const totalNarrations = sourcedDecisions.reduce((sum, { decision }) => sum + decision.narrations.length, 0);
  const narrationScores = evaluateNarrations(merged.narrations);
  const narrationLowScoreCount = narrationScores.filter(
    (entry) => entry.score < NARRATION_LOW_SCORE_THRESHOLD,
  ).length;

  narrationScores.forEach((entry, index) => {
    if (entry.score < NARRATION_LOW_SCORE_THRESHOLD) {
      logger.log(
        formatNarrationLowScoreWarning({
          evaluation: entry,
          metadata,
          index,
        }),
      );
    }
  });

  if (merged.commands.length > 0) {
    await publishCommands({
      source: "arbiter",
      commands: merged.commands,
      metadata,
    });
  }
  if (merged.narrations.length > 0) {
    await publishNarrations({
      narrations: merged.narrations,
      metadata,
    });
  }

  if (worldModel && merged.currentEra) {
    worldModel.setCurrentEra(merged.currentEra);
  }

  if (worldModel) {
    for (const { source, decision } of sourcedDecisions) {
      worldModel.recordDecision(source, decision);
    }
  }

  logger.log(
    `[tiandao] === tick end === commands: ${merged.commands.length}, narrations: ${merged.narrations.length}, correlation_id: ${metadata.correlationId}\n`,
  );

  const timeoutCount = results.reduce((count, result) => {
    if (
      result.status === "rejected" &&
      toAgentTickError(result.reason).error instanceof LlmTimeoutError
    ) {
      return count + 1;
    }

    return count;
  }, 0);
  const llmBackoffCount = results.reduce((count, result) => {
    if (
      result.status === "rejected" &&
      toAgentTickError(result.reason).error instanceof LlmBackoffError
    ) {
      return count + 1;
    }

    return count;
  }, 0);
  const errorBreakdown: TickErrorBreakdown = {
    ...emptyErrorBreakdown(),
    timeout: timeoutCount,
    backoff: (backoffCount ?? 0) + llmBackoffCount,
    parseFail: parseFailCount,
    reconnect: reconnectCount ?? 0,
    dedupeDrop: 0,
  };
  const metrics: TickMetrics = {
    tick: state.tick,
    timestamp: Date.now(),
    durationMs: Math.max(0, Date.now() - measuredTickStartMs),
    agentResults,
    mergedCommandCount: merged.commands.length,
    mergedNarrationCount: merged.narrations.length,
    chatSignalCount: chatSignalCount ?? chatSignals?.length ?? 0,
    eraChanged: merged.currentEra !== null,
    errorBreakdown,
    staleStateSkipped: staleStateSkipped ?? false,
    narrationScores,
    narrationLowScoreCount,
    narrationAverageScore: summarizeNarrationAverage(narrationScores),
  };

  if (telemetrySink) {
    try {
      await telemetrySink.recordTick(metrics);
    } catch (error) {
      (telemetryWarnLogger ?? console).warn("[tiandao] telemetry recordTick failed:", error);
    }
  }

  return {
    totalCommands,
    totalNarrations,
    skipped: sourcedDecisions.length === 0,
    metadata,
    metrics,
  };
}

async function runFreshTickWithRollback(args: {
    worldModel: WorldModel;
    run: () => Promise<void>;
  }): Promise<void> {
  const { worldModel, run } = args;
  const rollbackSnapshot = worldModel.toJSON();

  try {
    await run();
  } catch (error) {
    worldModel.restoreFromJSON(rollbackSnapshot);
    throw error;
  }
}

function defaultSleep(ms: number): Promise<void> {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
}

export function computeLoopBackoffMs(failureStreak: number): number {
  if (failureStreak <= 0) {
    return LOOP_BACKOFF_BASE_MS;
  }
  const exponential = LOOP_BACKOFF_BASE_MS * 2 ** (failureStreak - 1);
  return Math.min(exponential, LOOP_BACKOFF_MAX_MS);
}

function applyChatSignalsToAgents(agents: TickAgent[], chatSignals: ChatSignal[]): void {
  for (const agent of agents) {
    if (typeof agent.setChatSignals === "function") {
      agent.setChatSignals(chatSignals);
    }
  }
}

function applyWorldModelToAgents(agents: TickAgent[], worldModel?: WorldModel): void {
  if (!worldModel) {
    return;
  }

  for (const agent of agents) {
    if (typeof agent.setWorldModel === "function") {
      agent.setWorldModel(worldModel);
    }
  }
}

function resolveTickAgentRole(name: string): TickAgentRole {
  if (name === "calamity" || name === "mutation" || name === "era") {
    return name;
  }

  return "default";
}

export async function runRuntime(
  config: RuntimeConfig,
  deps: RuntimeDeps = {},
): Promise<void> {
  const logger = deps.logger ?? console;
  const modelOverrides = resolveModelOverrides(config);
  const agents = deps.agents ?? createDefaultAgents({ modelOverrides });
  const llmClientsByRole = createRuntimeClients(
    {
      ...config,
      modelOverrides,
      model: modelOverrides.default,
    },
    {
      createClient: deps.createClient,
      createMockClient: deps.createMockClient,
    },
  );
  const llmClient = llmClientsByRole.default;
  const annotateClient = llmClientsByRole.annotate;
  const annotateModel = modelOverrides.annotate;
  const worldModel = deps.worldModel ?? new WorldModel();
  const sleep = deps.sleep ?? defaultSleep;
  const telemetrySink = deps.telemetrySink ?? createDefaultTelemetrySink({ logger });

  logger.log(
    `[tiandao] models: default=${modelOverrides.default}, annotate=${modelOverrides.annotate}, calamity=${modelOverrides.calamity}, mutation=${modelOverrides.mutation}, era=${modelOverrides.era}, base_url: ${config.baseUrl ?? "(mock/no-remote)"}`,
  );
  logger.log(`[tiandao] mode: ${config.mockMode ? "mock (single-tick)" : "redis (loop)"}`);

  if (config.mockMode) {
    const state = deps.loadMockState ? deps.loadMockState() : createMockWorldState();
    await runTick(state, {
      agents,
      llmClient,
      model: modelOverrides.default,
      llmClientsByRole,
      modelOverrides,
      worldModel,
      publishCommands: async () => {},
      publishNarrations: async () => {},
      logger,
      tickStartedAtMs: Date.now(),
      reconnectCount: 0,
      backoffCount: 0,
      staleStateSkipped: false,
      chatSignalCount: 0,
      telemetrySink,
      telemetryWarnLogger: logger,
    });
    try {
      await telemetrySink.flush();
    } catch (error) {
      logger.warn("[tiandao] telemetry flush failed:", error);
    }
    return;
  }

  const createRedis = deps.createRedis ?? ((url: string) => new RedisIpc({ url }));
  const redis = createRedis(config.redisUrl);

  let running = true;
  let connected = false;
  let failureStreak = 0;
  let latestChatSignals: ChatSignal[] = [];
  let loopIterations = 0;
  let lastProcessedStateCursor: WorldStateCursor | null = null;
  const maxLoopIterations = deps.maxLoopIterations ?? Number.POSITIVE_INFINITY;
  let hasConnectedAtLeastOnce = false;
  let pendingReconnectCount = 0;
  let pendingBackoffCount = 0;
  let pendingStaleSkip = false;

  const shutdown = () => {
    logger.log("\n[tiandao] shutting down...");
    running = false;
  };

  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
  logger.log("[tiandao] waiting for world state from server...");

  try {
    while (running && loopIterations < maxLoopIterations) {
      loopIterations += 1;
      const tickStartedAt = Date.now();
      const nowSeconds = Math.floor(tickStartedAt / 1000);
      latestChatSignals = mergeChatSignals(latestChatSignals, [], nowSeconds);

      try {
        if (!connected) {
          await redis.connect();
          if (hasConnectedAtLeastOnce) {
            pendingReconnectCount += 1;
          }
          hasConnectedAtLeastOnce = true;
          connected = true;
          logger.log(`[tiandao] connected to Redis at ${redactRedisUrlForLog(config.redisUrl)}`);

        }

        const drainedChat = await redis.drainPlayerChat({
          maxItems: CHAT_DRAIN_WINDOW,
          logger,
        });

        if (drainedChat.length > 0) {
          try {
            const annotatedSignals = await processChatBatch({
              messages: drainedChat,
              annotateClient,
              annotateModel,
              logger,
            });
            latestChatSignals = mergeChatSignals(
              latestChatSignals,
              annotatedSignals,
              nowSeconds,
            );
            logger.log(
              `[tiandao] chat drain: messages=${drainedChat.length}, signals=${latestChatSignals.length}`,
            );
          } catch (error) {
            logger.warn("[tiandao] chat signal processing failed, keeping previous snapshot:", error);
          }
        }

        const state = redis.getLatestState();
        if (state) {
          if (isStaleWorldState(state, lastProcessedStateCursor)) {
            logger.log(
              `[tiandao] stale_state_skip tick=${state.tick} last_processed_tick=${lastProcessedStateCursor?.tick ?? "(none)"}`,
            );
            pendingStaleSkip = true;
          } else {
            await runFreshTickWithRollback({
              worldModel,
              run: async () => {
                await runTick(state, {
                  agents,
                  llmClient,
                  model: modelOverrides.default,
                  llmClientsByRole,
                  modelOverrides,
                  chatSignals: latestChatSignals,
                  worldModel,
                  publishCommands: (request) => redis.publishCommands(request),
                  publishNarrations: (request) => redis.publishNarrations(request),
                  logger,
                  tickStartedAtMs: tickStartedAt,
                  reconnectCount: pendingReconnectCount,
                  backoffCount: pendingBackoffCount,
                  staleStateSkipped: pendingStaleSkip,
                  chatSignalCount: latestChatSignals.length,
                  telemetrySink,
                  telemetryWarnLogger: logger,
                });
                await persistWorldModelAfterFreshTick({
                  worldModel,
                  redis,
                  logger,
                  metadata: {
                    sourceTick: state.tick,
                    correlationId: `tiandao-tick-${state.tick}`,
                  },
                });
              },
            });
            lastProcessedStateCursor = {
              tick: state.tick,
              ts: state.ts,
            };
            pendingReconnectCount = 0;
            pendingBackoffCount = 0;
            pendingStaleSkip = false;
          }
        }

        const elapsedMs = Date.now() - tickStartedAt;
        logger.log(`[tiandao] loop tick took ${elapsedMs}ms`);

        if (failureStreak > 0) {
          logger.log(`[tiandao] recovered after ${failureStreak} transient failure(s)`);
          failureStreak = 0;
        }

        await sleep(TICK_INTERVAL_MS);
      } catch (error) {
        failureStreak += 1;
        connected = false;
        pendingBackoffCount += 1;
        const backoffMs = computeLoopBackoffMs(failureStreak);
        logger.warn(
          `[tiandao] transient loop failure #${failureStreak}, backing off ${backoffMs}ms`,
          error,
        );
        await sleep(backoffMs);
      }
    }
  } finally {
    process.off("SIGINT", shutdown);
    process.off("SIGTERM", shutdown);

    try {
      try {
        await telemetrySink.flush();
      } catch (error) {
        logger.warn("[tiandao] telemetry flush failed:", error);
      }
      await redis.disconnect();
    } catch (error) {
      logger.warn("[tiandao] failed to disconnect Redis cleanly:", error);
    }

    logger.log("[tiandao] stopped");
  }
}

function isStaleWorldState(state: WorldStateV1, cursor: WorldStateCursor | null): boolean {
  if (!cursor) {
    return false;
  }

  if (cursor.ts !== null) {
    if (state.ts < cursor.ts) {
      return true;
    }

    if (state.ts > cursor.ts) {
      return false;
    }
  }

  return state.tick <= cursor.tick;
}

async function persistWorldModelAfterFreshTick(args: {
  worldModel: WorldModel;
  redis: RuntimeRedis;
  logger: Pick<typeof console, "warn">;
  metadata: TickPublishMetadata;
}): Promise<void> {
  const { worldModel, redis, logger, metadata } = args;
  const snapshot = worldModel.toJSON();
  if (snapshot.lastTick === null) {
    return;
  }

  try {
    await redis.publishAgentWorldModel?.({
      source: "arbiter",
      snapshot: worldModelSnapshotToEnvelopeSnapshot(snapshot),
      metadata,
    });
  } catch (error) {
    logger.warn("[tiandao] failed to publish world model snapshot:", error);
  }

  if (snapshot.lastTick % SNAPSHOT_INTERVAL_TICKS !== 0) {
    return;
  }

  try {
    await mkdir(getSnapshotDirPath(), { recursive: true });
    const filePath = resolve(
      getSnapshotDirPath(),
      `${SNAPSHOT_FILE_PREFIX}${snapshot.lastTick}${SNAPSHOT_FILE_SUFFIX}`,
    );
    await writeFile(filePath, `${JSON.stringify(snapshot, null, 2)}\n`, "utf8");
    await rotateSnapshotFiles(logger);
  } catch (error) {
    logger.warn("[tiandao] failed to persist world model snapshot file:", error);
  }
}

function worldModelSnapshotToEnvelopeSnapshot(
  snapshot: WorldModelSnapshot,
): AgentWorldModelEnvelopeV1["snapshot"] {
  return {
    currentEra: snapshot.currentEra,
    zoneHistory: snapshot.zoneHistory,
    lastDecisions: snapshot.lastDecisions,
    playerFirstSeenTick: snapshot.playerFirstSeenTick,
    lastTick: snapshot.lastTick,
    lastStateTs: snapshot.lastStateTs,
  };
}

interface SnapshotFileRecord {
  name: string;
  tick: number;
}

function getSnapshotDirPath(): string {
  return resolve(process.cwd(), "data");
}

async function listSnapshotFiles(): Promise<SnapshotFileRecord[]> {
  let entries: string[];
  try {
    entries = await readdir(getSnapshotDirPath());
  } catch {
    return [];
  }

  return entries
    .map((name) => {
      const match = name.match(/^tiandao-snapshot-(\d+)\.json$/);
      if (!match) {
        return null;
      }

      return {
        name,
        tick: Number(match[1]),
      };
    })
    .filter((entry): entry is SnapshotFileRecord => {
      return entry !== null && Number.isFinite(entry.tick);
    })
    .sort((left, right) => left.tick - right.tick);
}

async function rotateSnapshotFiles(logger: Pick<typeof console, "warn">): Promise<void> {
  const files = await listSnapshotFiles();
  if (files.length <= SNAPSHOT_KEEP_COUNT) {
    return;
  }

  const staleFiles = files.slice(0, files.length - SNAPSHOT_KEEP_COUNT);
  for (const stale of staleFiles) {
    const filePath = resolve(getSnapshotDirPath(), stale.name);
    try {
      await unlink(filePath);
    } catch (error) {
      logger.warn(`[tiandao] failed to remove stale snapshot ${stale.name}:`, error);
    }
  }
}

function createDefaultTelemetrySink(options: {
  logger: Pick<typeof console, "log" | "error" | "warn">;
}): TelemetrySink {
  return new FanoutTelemetrySink([
    new JsonLogSink({ logger: options.logger }),
    new RollingSummarySink({ logger: options.logger, intervalTicks: 10 }),
  ]);
}

class FanoutTelemetrySink implements TelemetrySink {
  constructor(private readonly sinks: TelemetrySink[]) {}

  async recordTick(metrics: TickMetrics): Promise<void> {
    if (this.sinks.length <= 0) {
      return;
    }

    await Promise.all(this.sinks.map((sink) => sink.recordTick(metrics)));
  }

  async flush(): Promise<void> {
    if (this.sinks.length <= 0) {
      return;
    }

    await Promise.all(this.sinks.map((sink) => sink.flush()));
  }
}

export { NoopTelemetrySink };

interface AgentTickError {
  error: unknown;
  startedAtMs: number;
  endedAtMs: number;
}

class AgentTickExecutionError extends Error {
  constructor(
    public readonly innerError: unknown,
    public readonly startedAtMs: number,
    public readonly endedAtMs: number,
  ) {
    super("Agent tick execution failed");
    this.name = "AgentTickExecutionError";
  }
}

function toAgentTickError(reason: unknown): AgentTickError {
  if (reason instanceof AgentTickExecutionError) {
    return {
      error: reason.innerError,
      startedAtMs: reason.startedAtMs,
      endedAtMs: reason.endedAtMs,
    };
  }

  if (
    typeof reason === "object" &&
    reason !== null &&
    "error" in reason &&
    "startedAtMs" in reason &&
    "endedAtMs" in reason
  ) {
    const typedReason = reason as {
      error: unknown;
      startedAtMs: unknown;
      endedAtMs: unknown;
    };
    const startedAtMs =
      typeof typedReason.startedAtMs === "number" ? typedReason.startedAtMs : Date.now();
    const endedAtMs = typeof typedReason.endedAtMs === "number" ? typedReason.endedAtMs : Date.now();
    return {
      error: typedReason.error,
      startedAtMs,
      endedAtMs,
    };
  }

  const now = Date.now();
  return {
    error: reason,
    startedAtMs: now,
    endedAtMs: now,
  };
}
