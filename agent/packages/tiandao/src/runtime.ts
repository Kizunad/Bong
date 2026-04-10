import type { ChatMessageV1, ChatSignal, Command, Narration, WorldStateV1 } from "@bong/schema";
import dotenv from "dotenv";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { TiandaoAgent } from "./agent.js";
import { CALAMITY_RECIPE, MUTATION_RECIPE, ERA_RECIPE } from "./context.js";
import { mergeChatSignals, processChatBatch } from "./chat-processor.js";
import { createClient, createMockClient, type LlmClient } from "./llm.js";
import { createMockWorldState } from "./mock-state.js";
import { Arbiter } from "./arbiter.js";
import { RedisIpc } from "./redis-ipc.js";
import type { AgentDecision } from "./parse.js";
import { WorldModel } from "./world-model.js";

export const MOCK_FLAG = "--mock";
export const DEFAULT_MODEL = "gpt-5.4-mini";
export const DEFAULT_REDIS_URL = "redis://127.0.0.1:6379";
const TICK_INTERVAL_MS = 5_000;
const CHAT_DRAIN_WINDOW = 128;
const LOOP_BACKOFF_BASE_MS = 1_000;
const LOOP_BACKOFF_MAX_MS = 30_000;

const __dirname = dirname(fileURLToPath(import.meta.url));

export interface RuntimeConfig {
  mockMode: boolean;
  model: string;
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
  publishCommands(source: "arbiter", commands: Command[]): Promise<void>;
  publishNarrations(narrations: Narration[]): Promise<void>;
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
}

export interface TickDeps {
  agents: TickAgent[];
  llmClient: LlmClient;
  model: string;
  chatSignals?: ChatSignal[];
  worldModel?: WorldModel;
  publishCommands: (source: "arbiter", commands: Command[]) => Promise<void>;
  publishNarrations: (narrations: Narration[]) => Promise<void>;
  logger: Pick<typeof console, "log" | "error">;
}

export function loadEnv(): void {
  dotenv.config({ path: resolve(__dirname, "../../../../.env") });
}

export function resolveRuntimeConfig(
  argv: string[] = process.argv,
  env: NodeJS.ProcessEnv = process.env,
): RuntimeConfig {
  const mockMode = argv.includes(MOCK_FLAG);
  return {
    mockMode,
    model: env["LLM_MODEL"] ?? DEFAULT_MODEL,
    redisUrl: env["REDIS_URL"] ?? DEFAULT_REDIS_URL,
    baseUrl: env["LLM_BASE_URL"] ?? null,
    apiKey: env["LLM_API_KEY"] ?? null,
  };
}

export function createDefaultAgents(now?: () => number): TiandaoAgent[] {
  return [
    new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 30_000,
      now,
    }),
    new TiandaoAgent({
      name: "mutation",
      skillFile: "mutation.md",
      recipe: MUTATION_RECIPE,
      intervalMs: 60_000,
      now,
    }),
    new TiandaoAgent({
      name: "era",
      skillFile: "era.md",
      recipe: ERA_RECIPE,
      intervalMs: 300_000,
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

export function createRuntimeClient(
  config: RuntimeConfig,
  deps: Pick<RuntimeDeps, "createClient" | "createMockClient"> = {},
): LlmClient {
  if (config.mockMode) {
    return (deps.createMockClient ?? createMockClient)();
  }

  const { baseUrl, apiKey } = requireLlmCredentials(config);

  const buildClient = deps.createClient ?? createClient;
  return buildClient({
    baseURL: baseUrl,
    apiKey,
    model: config.model,
  });
}

export async function runTick(state: WorldStateV1, deps: TickDeps): Promise<void> {
  const { agents, llmClient, model, chatSignals, worldModel, publishCommands, publishNarrations, logger } = deps;
  worldModel?.updateState(state);
  applyWorldModelToAgents(agents, worldModel);
  applyChatSignalsToAgents(agents, chatSignals ?? []);
  logger.log("[tiandao] === tick start ===");
  logger.log(
    `[tiandao] tick: ${state.tick}, players: ${state.players.length}, zones: ${state.zones.length}`,
  );

  const results = await Promise.allSettled(agents.map((agent) => agent.tick(llmClient, model, state)));

  const sourcedDecisions: Array<{ source: string; decision: AgentDecision }> = [];

  for (let i = 0; i < results.length; i++) {
    const result = results[i];
    const agent = agents[i];
    if (result.status === "fulfilled" && result.value) {
      logger.log(
        `[tiandao][${agent.name}] commands: ${result.value.commands.length}, narrations: ${result.value.narrations.length}`,
      );
      logger.log(`[tiandao][${agent.name}] reasoning: ${result.value.reasoning}`);
      sourcedDecisions.push({ source: agent.name, decision: result.value });
    } else if (result.status === "rejected") {
      logger.error(`[tiandao][${agent.name}] error:`, result.reason);
    } else {
      logger.log(`[tiandao][${agent.name}] skipped (not due yet)`);
    }
  }

  const merged = new Arbiter(state).merge(sourcedDecisions);
  if (merged.commands.length > 0) {
    await publishCommands("arbiter", merged.commands);
  }
  if (merged.narrations.length > 0) {
    await publishNarrations(merged.narrations);
  }

  if (worldModel && merged.currentEra) {
    worldModel.setCurrentEra(merged.currentEra);
  }

  if (worldModel) {
    for (const { source, decision } of sourcedDecisions) {
      worldModel.recordDecision(source, decision);
    }
  }

  const totalCommands = merged.commands.length;
  const totalNarrations = merged.narrations.length;
  logger.log(`[tiandao] === tick end === commands: ${totalCommands}, narrations: ${totalNarrations}\n`);
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

export async function runRuntime(
  config: RuntimeConfig,
  deps: RuntimeDeps = {},
): Promise<void> {
  const logger = deps.logger ?? console;
  const agents = deps.agents ?? createDefaultAgents();
  const llmClient = createRuntimeClient(config, {
    createClient: deps.createClient,
    createMockClient: deps.createMockClient,
  });
  const worldModel = deps.worldModel ?? new WorldModel();
  const sleep = deps.sleep ?? defaultSleep;

  logger.log(
    `[tiandao] model: ${config.model}, base_url: ${config.baseUrl ?? "(mock/no-remote)"}`,
  );
  logger.log(`[tiandao] mode: ${config.mockMode ? "mock (single-tick)" : "redis (loop)"}`);

  if (config.mockMode) {
    const state = deps.loadMockState ? deps.loadMockState() : createMockWorldState();
    await runTick(state, {
      agents,
      llmClient,
      model: config.model,
      worldModel,
      publishCommands: async () => {},
      publishNarrations: async () => {},
      logger,
    });
    return;
  }

  const createRedis = deps.createRedis ?? ((url: string) => new RedisIpc({ url }));
  const redis = createRedis(config.redisUrl);

  let running = true;
  let connected = false;
  let failureStreak = 0;
  let latestChatSignals: ChatSignal[] = [];
  let loopIterations = 0;
  const maxLoopIterations = deps.maxLoopIterations ?? Number.POSITIVE_INFINITY;

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
          connected = true;
          logger.log(`[tiandao] connected to Redis at ${config.redisUrl}`);
        }

        const drainedChat = await redis.drainPlayerChat({
          maxItems: CHAT_DRAIN_WINDOW,
          logger,
        });

        if (drainedChat.length > 0) {
          try {
            const annotatedSignals = await processChatBatch({
              messages: drainedChat,
              llmClient,
              model: config.model,
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
          await runTick(state, {
            agents,
            llmClient,
            model: config.model,
            chatSignals: latestChatSignals,
            worldModel,
            publishCommands: (source, commands) => redis.publishCommands(source, commands),
            publishNarrations: (narrations) => redis.publishNarrations(narrations),
            logger,
          });
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
      await redis.disconnect();
    } catch (error) {
      logger.warn("[tiandao] failed to disconnect Redis cleanly:", error);
    }

    logger.log("[tiandao] stopped");
  }
}
