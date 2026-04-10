import { fileURLToPath } from "node:url";
import type { Command, Narration, WorldStateV1 } from "@bong/schema";
import { TiandaoAgent } from "./agent.js";
import { Arbiter } from "./arbiter.js";
import { CALAMITY_RECIPE, MUTATION_RECIPE, ERA_RECIPE } from "./context.js";
import { createMockClient, createClient, type LlmClient, type LlmClientConfig } from "./llm.js";
import { createMockWorldState } from "./mock-state.js";
import type { AgentDecision } from "./parse.js";
import { loadEnv, resolveRuntimeConfig, runRuntime, createDefaultAgents } from "./runtime.js";
import { WorldModel } from "./world-model.js";

const MOCK_COMPLETION_MARKER = "[tiandao] mock tick complete";

export interface PublishSink {
  publishCommands(source: string, commands: Command[]): Promise<void>;
  publishNarrations(narrations: Narration[]): Promise<void>;
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
  worldModel.updateState(state);

  for (const agent of agents) {
    agent.setWorldModel(worldModel);
  }

  const results = await Promise.allSettled(
    agents.map((agent) => agent.tick(llmClient, model, state)),
  );

  const sourcedDecisions: Array<{ source: string; decision: AgentDecision }> = [];
  let totalCommands = 0;
  let totalNarrations = 0;

  for (let i = 0; i < results.length; i++) {
    const result = results[i];
    if (result.status === "fulfilled" && result.value) {
      sourcedDecisions.push({ source: agents[i].name, decision: result.value });
      totalCommands += result.value.commands.length;
      totalNarrations += result.value.narrations.length;
    }
  }

  const merged = new Arbiter(state).merge(sourcedDecisions);

  if (sink) {
    if (merged.commands.length > 0) {
      await sink.publishCommands("merged", merged.commands);
    }
    if (merged.narrations.length > 0) {
      await sink.publishNarrations(merged.narrations);
    }
  }

  if (merged.currentEra) {
    worldModel.setCurrentEra(merged.currentEra);
  }

  for (const { source, decision } of sourcedDecisions) {
    worldModel.recordDecision(source, decision);
  }

  return {
    totalCommands,
    totalNarrations,
    chatSignalCount: 0,
    skipped: sourcedDecisions.length === 0,
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
  await runRuntime(config);
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
