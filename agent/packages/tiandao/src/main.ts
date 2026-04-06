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

import { createClient } from "./llm.js";
import { TiandaoAgent } from "./agent.js";
import { CALAMITY_RECIPE, MUTATION_RECIPE, ERA_RECIPE } from "./context.js";
import { createMockWorldState } from "./mock-state.js";
import { RedisIpc } from "./redis-ipc.js";
import type { WorldStateV1 } from "@bong/schema";
import type { AgentDecision } from "./parse.js";

const BASE_URL = process.env["LLM_BASE_URL"];
const API_KEY = process.env["LLM_API_KEY"];
const MODEL = process.env["LLM_MODEL"] ?? "gpt-5.4-mini";
const REDIS_URL = process.env["REDIS_URL"] ?? "redis://127.0.0.1:6379";
const MOCK_MODE = process.argv.includes("--mock");
const TICK_INTERVAL_MS = 5_000; // main loop minimum interval

if (!BASE_URL || !API_KEY) {
  console.error("Missing LLM_BASE_URL or LLM_API_KEY in .env");
  process.exit(1);
}

const llmClient = createClient({ baseURL: BASE_URL, apiKey: API_KEY, model: MODEL });

const agents = [
  new TiandaoAgent({
    name: "calamity",
    skillFile: "calamity.md",
    recipe: CALAMITY_RECIPE,
    intervalMs: 30_000,
  }),
  new TiandaoAgent({
    name: "mutation",
    skillFile: "mutation.md",
    recipe: MUTATION_RECIPE,
    intervalMs: 60_000,
  }),
  new TiandaoAgent({
    name: "era",
    skillFile: "era.md",
    recipe: ERA_RECIPE,
    intervalMs: 300_000,
  }),
];

async function runTick(
  state: WorldStateV1,
  redis: RedisIpc | null,
): Promise<void> {
  console.log("[tiandao] === tick start ===");
  console.log(`[tiandao] tick: ${state.tick}, players: ${state.players.length}, zones: ${state.zones.length}`);

  const results = await Promise.allSettled(
    agents.map((a) => a.tick(llmClient, MODEL, state)),
  );

  const decisions: AgentDecision[] = [];

  for (let i = 0; i < results.length; i++) {
    const result = results[i];
    const agent = agents[i];
    if (result.status === "fulfilled" && result.value) {
      console.log(`[tiandao][${agent.name}] commands: ${result.value.commands.length}, narrations: ${result.value.narrations.length}`);
      console.log(`[tiandao][${agent.name}] reasoning: ${result.value.reasoning}`);
      decisions.push(result.value);

      // Publish to Redis if connected
      if (redis) {
        await redis.publishCommands(agent.name, result.value.commands);
        await redis.publishNarrations(result.value.narrations);
      }
    } else if (result.status === "rejected") {
      console.error(`[tiandao][${agent.name}] error:`, result.reason);
    } else {
      console.log(`[tiandao][${agent.name}] skipped (not due yet)`);
    }
  }

  const totalCommands = decisions.reduce((n, d) => n + d.commands.length, 0);
  const totalNarrations = decisions.reduce((n, d) => n + d.narrations.length, 0);
  console.log(`[tiandao] === tick end === commands: ${totalCommands}, narrations: ${totalNarrations}\n`);
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

async function main(): Promise<void> {
  console.log(`[tiandao] model: ${MODEL}, base_url: ${BASE_URL}`);
  console.log(`[tiandao] mode: ${MOCK_MODE ? "mock (single-tick)" : "redis (loop)"}`);

  if (MOCK_MODE) {
    // Single tick with mock data
    await runTick(createMockWorldState(), null);
    return;
  }

  // Redis loop mode
  const redis = new RedisIpc({ url: REDIS_URL });
  await redis.connect();
  console.log(`[tiandao] connected to Redis at ${REDIS_URL}`);

  // Graceful shutdown
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
      await runTick(state, redis);
    }

    await sleep(TICK_INTERVAL_MS);
  }

  await redis.disconnect();
  console.log("[tiandao] stopped");
}

main().catch((err) => {
  console.error("[tiandao] fatal:", err);
  process.exit(1);
});
