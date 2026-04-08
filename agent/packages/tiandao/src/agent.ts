/**
 * 天道 Agent — 单个 Agent 实例
 * 加载 skill prompt + 拼装上下文 + 调用 LLM + 解析决策
 */

import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type { WorldStateV1 } from "@bong/schema";
import { type ContextRecipe, assembleContext } from "./context.js";
import type { LlmClient, LlmMessage } from "./llm.js";
import { type AgentDecision, parseDecision } from "./parse.js";
import type { WorldModel } from "./world-model.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

export interface AgentConfig {
  name: string;
  skillFile: string; // relative to skills/
  recipe: ContextRecipe;
  intervalMs: number;
  now?: () => number;
}

export class TiandaoAgent {
  readonly name: string;
  private systemPrompt: string;
  private recipe: ContextRecipe;
  private lastRunTs = 0;
  readonly intervalMs: number;
  private readonly now: () => number;

  constructor(config: AgentConfig) {
    this.name = config.name;
    this.recipe = config.recipe;
    this.intervalMs = config.intervalMs;
    this.now = config.now ?? (() => Date.now());
    this.systemPrompt = readFileSync(
      resolve(__dirname, "skills", config.skillFile),
      "utf-8",
    );
  }

  shouldRun(now: number): boolean {
    return now - this.lastRunTs >= this.intervalMs;
  }

  async tick(
    client: LlmClient,
    model: string,
    state: WorldStateV1,
    worldModel?: WorldModel,
  ): Promise<AgentDecision | null> {
    const now = this.now();
    if (!this.shouldRun(now)) return null;

    this.lastRunTs = now;

    const context = assembleContext(this.recipe, state, { worldModel });
    const userPrompt = `${context}\n\n---\n\n请基于以上信息决策。输出 JSON。如果不需要行动，返回空数组。`;

    console.log(`[tiandao][${this.name}] thinking...`);

    const messages: LlmMessage[] = [
      { role: "system", content: this.systemPrompt },
      { role: "user", content: userPrompt },
    ];

    const raw = await client.chat(model, messages);

    console.log(`[tiandao][${this.name}] response:\n${raw}\n`);

    const decision = parseDecision(raw);
    decision.commands.forEach((cmd) => {
      // Tag source for arbiter
      (cmd as Record<string, unknown>)._source = this.name;
    });

    return decision;
  }
}
