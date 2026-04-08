/**
 * 天道 Agent 主循环
 * --mock  : 使用 mock world state 单次运行（测试用）
 * 默认    : 接 Redis IPC，持续循环推演
 */

import dotenv from "dotenv";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
dotenv.config({ path: resolve(__dirname, "../../../../.env") });

import { TiandaoAgent } from "./agent.js";
import { Arbiter } from "./arbiter.js";
import { CALAMITY_RECIPE, MUTATION_RECIPE, ERA_RECIPE } from "./context.js";
import { LlmBackoffError, LlmTimeoutError, createClient, createMockClient, type LlmClient } from "./llm.js";
import { createMockWorldState } from "./mock-state.js";
import type { AgentDecision } from "./parse.js";
import { RedisIpc } from "./redis-ipc.js";
import type { Command, Narration, WorldStateV1 } from "@bong/schema";
import { WorldModel, type CurrentEraSnapshot } from "./world-model.js";

const MOCK_MODE = process.argv.includes("--mock");
const TICK_INTERVAL_MS = 5_000;
const MOCK_COMPLETION_MARKER = "[tiandao] mock tick complete: deterministic";

export interface PublishSink {
  publishCommands(source: string, commands: Command[]): Promise<void>;
  publishNarrations(narrations: Narration[]): Promise<void>;
}

export interface RuntimeConfig {
  baseUrl?: string;
  apiKey?: string;
  model?: string;
  redisUrl?: string;
  mockMode?: boolean;
}

export interface TickSummary {
  totalCommands: number;
  totalNarrations: number;
  durationMs: number;
  chatSignalCount: number;
  skipped: boolean;
  decisions: AgentDecision[];
}

interface GuardStateReader {
  getGuardState(): {
    consecutiveFailures: number;
    backoffUntil: number;
  };
}

interface EraAnnouncementParseResult {
  era: CurrentEraSnapshot;
  confidence: number;
}

function isGuardStateReader(client: LlmClient): client is LlmClient & GuardStateReader {
  const candidate = client as Partial<GuardStateReader>;
  return typeof candidate.getGuardState === "function";
}

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return `${error.name}: ${error.message}`;
  }
  return String(error);
}

function deduplicateNarrations(narrations: Narration[]): Narration[] {
  const seen = new Set<string>();
  const result: Narration[] = [];

  for (const narration of narrations) {
    const key = JSON.stringify(narration);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(narration);
  }

  return result;
}

function parseEraAnnouncementFromNarrationText(text: string): EraAnnouncementParseResult | null {
  const normalized = text
    .replace(/[“”"「」]/g, "")
    .replace(/[：:]/g, " ")
    .replace(/[\s\t\n]+/g, " ")
    .trim();
  if (!normalized) return null;

  const patterns: Array<{ regex: RegExp; confidence: number }> = [
    {
      regex: /(?:宣告|昭告|进入|步入)(?:新)?([\u4e00-\u9fa5A-Za-z0-9_\-]{2,20}(?:纪|时代|元年|天运))/,
      confidence: 3,
    },
    {
      regex: /(?:当前|今后|此后|现世)(?:进入|步入|属于)([\u4e00-\u9fa5A-Za-z0-9_\-]{2,20}(?:纪|时代|元年|天运))/,
      confidence: 2,
    },
    {
      regex: /([\u4e00-\u9fa5A-Za-z0-9_\-]{2,20}(?:纪|时代|元年|天运))(?:已至|降临|来临|开启)/,
      confidence: 1,
    },
  ];

  for (const pattern of patterns) {
    const match = normalized.match(pattern.regex);
    if (!match?.[1]) {
      continue;
    }

    const name = match[1].trim();
    if (!name) {
      continue;
    }

    const truncatedEffect = normalized.slice(0, 120);
    return {
      era: {
        name,
        sinceTick: 0,
        globalEffect: truncatedEffect,
      },
      confidence: pattern.confidence,
    };
  }

  return null;
}

function pickEraAnnouncementFromDecision(decision: AgentDecision): CurrentEraSnapshot | null {
  const candidates: EraAnnouncementParseResult[] = [];

  for (const narration of decision.narrations) {
    if (narration.style !== "era_decree") {
      continue;
    }

    const parsed = parseEraAnnouncementFromNarrationText(narration.text);
    if (parsed) {
      candidates.push(parsed);
    }
  }

  if (candidates.length === 0) {
    return null;
  }

  candidates.sort((left, right) => {
    if (right.confidence !== left.confidence) {
      return right.confidence - left.confidence;
    }

    if (left.era.name !== right.era.name) {
      return left.era.name.localeCompare(right.era.name);
    }

    return left.era.globalEffect.localeCompare(right.era.globalEffect);
  });

  return candidates[0]?.era ?? null;
}

function applyEraMemory(
  worldModel: WorldModel,
  previous: CurrentEraSnapshot | null,
  announced: CurrentEraSnapshot | null,
  tick: number,
): CurrentEraSnapshot | null {
  if (!announced) {
    return previous;
  }

  if (previous && previous.name === announced.name && previous.globalEffect === announced.globalEffect) {
    return previous;
  }

  const next: CurrentEraSnapshot = {
    name: announced.name,
    sinceTick: tick,
    globalEffect: announced.globalEffect,
  };
  worldModel.rememberCurrentEra(next);
  return next;
}

function createAgents(now?: () => number): TiandaoAgent[] {
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

async function runTickWithDeps(
  deps: { agents: TiandaoAgent[]; llmClient: LlmClient; model: string; worldModel: WorldModel },
  state: WorldStateV1,
  sink: PublishSink | null,
): Promise<TickSummary> {
  const startedAt = Date.now();
  const chatSignalCount = 0;

  deps.worldModel.updateState(state);

  console.log("[tiandao] === tick start ===");
  console.log(
    `[tiandao] tick: ${state.tick}, players: ${state.players.length}, zones: ${state.zones.length}`,
  );

  const results = await Promise.allSettled(
    deps.agents.map((agent) => agent.tick(deps.llmClient, deps.model, state, deps.worldModel)),
  );

  const decisions: AgentDecision[] = [];
  const decisionsByAgent = new Map<string, AgentDecision>();
  let announcedEra: CurrentEraSnapshot | null = null;
  let skippedByBackoff = 0;

  for (let i = 0; i < results.length; i++) {
    const result = results[i];
    const agent = deps.agents[i];
    if (result.status === "fulfilled" && result.value) {
      console.log(
        `[tiandao][${agent.name}] commands: ${result.value.commands.length}, narrations: ${result.value.narrations.length}`,
      );
      console.log(`[tiandao][${agent.name}] reasoning: ${result.value.reasoning}`);
      const normalized: AgentDecision = {
        ...result.value,
        narrations: deduplicateNarrations(result.value.narrations),
      };
      decisions.push(normalized);
      decisionsByAgent.set(agent.name, normalized);

      if (agent.name === "era") {
        announcedEra = pickEraAnnouncementFromDecision(normalized);
      }
    } else if (result.status === "rejected") {
      if (result.reason instanceof LlmBackoffError) {
        skippedByBackoff += 1;
        console.warn(`[tiandao][${agent.name}] skipped (llm backoff)`, {
          retry_at: result.reason.retryAt,
        });
        continue;
      }

      if (result.reason instanceof LlmTimeoutError) {
        console.warn(`[tiandao][${agent.name}] skipped (llm timeout)`, {
          reason: result.reason.message,
        });
        continue;
      }

      console.warn(`[tiandao][${agent.name}] failed`, {
        reason: formatError(result.reason),
      });
    } else {
      console.log(`[tiandao][${agent.name}] skipped (not due yet)`);
    }
  }

  deps.worldModel.rememberDecisions(decisionsByAgent.entries());
  const previousEra = deps.worldModel.currentEra;
  const updatedEra = applyEraMemory(deps.worldModel, previousEra, announcedEra, state.tick);
  if (updatedEra && (!previousEra || previousEra.name !== updatedEra.name || previousEra.globalEffect !== updatedEra.globalEffect)) {
    console.log("[tiandao][era] current era updated", {
      name: updatedEra.name,
      sinceTick: updatedEra.sinceTick,
      globalEffect: updatedEra.globalEffect,
    });
  }

  if (isGuardStateReader(deps.llmClient)) {
    const guardState = deps.llmClient.getGuardState();
    if (guardState.backoffUntil > Date.now()) {
      console.warn("[tiandao][llm] guard backoff active", {
        consecutive_failures: guardState.consecutiveFailures,
        retry_at: guardState.backoffUntil,
      });
    }
  }

  const arbiter = new Arbiter();
  const merged = arbiter.merge(decisions, state);
  console.log(
    `[tiandao][arbiter] merged commands: ${merged.commands.length}, narrations: ${merged.narrations.length}`,
  );

  if (sink) {
    await sink.publishCommands("merged", merged.commands);
    await sink.publishNarrations(merged.narrations);
  }

  const totalCommands = decisions.reduce((sum, item) => sum + item.commands.length, 0);
  const totalNarrations = decisions.reduce((sum, item) => sum + item.narrations.length, 0);
  const durationMs = Date.now() - startedAt;
  const skipped = decisions.length === 0;

  console.log("[tiandao][tick-metrics]", {
    duration_ms: durationMs,
    command_count: totalCommands,
    narration_count: totalNarrations,
    chat_signal_count: chatSignalCount,
    skipped,
    skipped_by_backoff: skippedByBackoff,
  });

  console.log(
    `[tiandao] === tick end === commands: ${totalCommands}, narrations: ${totalNarrations}\n`,
  );

  return {
    totalCommands,
    totalNarrations,
    durationMs,
    chatSignalCount,
    skipped,
    decisions,
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
}

export async function runMockTickForTest(options?: {
  llmClient?: LlmClient;
  state?: WorldStateV1;
  sink?: PublishSink | null;
  now?: () => number;
  model?: string;
  worldModel?: WorldModel;
}): Promise<TickSummary> {
  const model = options?.model ?? "gpt-5.4-mini";
  const llmClient = options?.llmClient ?? createMockClient();
  const state = options?.state ?? createMockWorldState();
  const agents = createAgents(options?.now ?? (() => 1_000_000));
  const worldModel = options?.worldModel ?? new WorldModel();
  return runTickWithDeps({ agents, llmClient, model, worldModel }, state, options?.sink ?? null);
}

export function getMockCompletionMarker(): string {
  return MOCK_COMPLETION_MARKER;
}

export async function main(runtimeConfig: RuntimeConfig = {}): Promise<void> {
  const baseUrl = runtimeConfig.baseUrl ?? process.env.LLM_BASE_URL;
  const apiKey = runtimeConfig.apiKey ?? process.env.LLM_API_KEY;
  const model = runtimeConfig.model ?? process.env.LLM_MODEL ?? "gpt-5.4-mini";
  const redisUrl = runtimeConfig.redisUrl ?? process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
  const mockMode = runtimeConfig.mockMode ?? MOCK_MODE;

  console.log(`[tiandao] model: ${model}, base_url: ${baseUrl ?? "<unset>"}`);
  console.log(`[tiandao] mode: ${mockMode ? "mock (single-tick)" : "redis (loop)"}`);

  if (mockMode) {
    await runMockTickForTest({ model });
    console.log(MOCK_COMPLETION_MARKER);
    return;
  }

  if (!baseUrl || !apiKey) {
    console.error("Missing LLM_BASE_URL or LLM_API_KEY in .env");
    process.exit(1);
  }

  const llmClient = createClient({ baseURL: baseUrl, apiKey, model });
  const agents = createAgents();
  const worldModel = new WorldModel();
  const redis = new RedisIpc({ url: redisUrl });
  await redis.connect();
  console.log(`[tiandao] connected to Redis at ${redisUrl}`);

  let running = true;
  const shutdown = () => {
    console.log("\n[tiandao] shutting down...");
    running = false;
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);

  console.log("[tiandao] waiting for world state from server...");

  while (running) {
    const state = redis.getLatestState();

    if (state) {
      await runTickWithDeps({ agents, llmClient, model, worldModel }, state, redis);
    }

    await sleep(TICK_INTERVAL_MS);
  }

  await redis.disconnect();
  console.log("[tiandao] stopped");
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((err) => {
    console.error("[tiandao] fatal:", err);
    process.exit(1);
  });
}
