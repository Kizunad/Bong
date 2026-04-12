import { fileURLToPath } from "node:url";
import type { Command, Narration } from "@bong/schema";
import { createMockClient, type LlmClient } from "./llm.js";
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
