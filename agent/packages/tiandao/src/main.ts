/**
 * 天道 Agent 主循环
 * MVP: mock world state → 3 agents 并发推演 → 打印结果
 * 后续: 接 Redis IPC
 */

import dotenv from "dotenv";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
// Load .env from project root (Bong/)
dotenv.config({ path: resolve(__dirname, "../../../../.env") });
import { createClient } from "./llm.js";
import { TiandaoAgent } from "./agent.js";
import { CALAMITY_RECIPE, MUTATION_RECIPE, ERA_RECIPE } from "./context.js";
import { createMockWorldState } from "./mock-state.js";
import type { AgentDecision } from "./parse.js";

const BASE_URL = process.env["LLM_BASE_URL"];
const API_KEY = process.env["LLM_API_KEY"];
const MODEL = process.env["LLM_MODEL"] ?? "gpt-5.4-mini";

if (!BASE_URL || !API_KEY) {
  console.error("Missing LLM_BASE_URL or LLM_API_KEY in .env");
  process.exit(1);
}

const client = createClient({ baseURL: BASE_URL, apiKey: API_KEY, model: MODEL });

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

async function runOnce(): Promise<void> {
  const state = createMockWorldState();

  console.log("[tiandao] === tick start ===");
  console.log(`[tiandao] players: ${state.players.length}, zones: ${state.zones.length}`);

  const results = await Promise.allSettled(
    agents.map((a) => a.tick(client, MODEL, state)),
  );

  const decisions: AgentDecision[] = [];

  for (let i = 0; i < results.length; i++) {
    const result = results[i];
    const agent = agents[i];
    if (result.status === "fulfilled" && result.value) {
      console.log(`[tiandao][${agent.name}] commands: ${result.value.commands.length}, narrations: ${result.value.narrations.length}`);
      console.log(`[tiandao][${agent.name}] reasoning: ${result.value.reasoning}`);
      decisions.push(result.value);
    } else if (result.status === "rejected") {
      console.error(`[tiandao][${agent.name}] error:`, result.reason);
    } else {
      console.log(`[tiandao][${agent.name}] skipped (not due yet)`);
    }
  }

  const totalCommands = decisions.reduce((n, d) => n + d.commands.length, 0);
  const totalNarrations = decisions.reduce((n, d) => n + d.narrations.length, 0);
  console.log(`[tiandao] === tick end === commands: ${totalCommands}, narrations: ${totalNarrations}`);
}

// MVP: 单次运行
console.log("[tiandao] starting single-tick test...");
console.log(`[tiandao] model: ${MODEL}, base_url: ${BASE_URL}`);
runOnce()
  .then(() => console.log("[tiandao] done"))
  .catch((err) => console.error("[tiandao] fatal:", err));
