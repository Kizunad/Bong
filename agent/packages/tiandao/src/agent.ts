/**
 * 天道 Agent — 单个 Agent 实例
 * 加载 skill prompt + 拼装上下文 + 调用 LLM + 解析决策
 */

import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type OpenAI from "openai";
import type { WorldStateV1 } from "@bong/schema";
import { type ContextRecipe, assembleContext } from "./context.js";
import { chat } from "./llm.js";
import { type AgentDecision, parseDecision } from "./parse.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

export interface AgentConfig {
  name: string;
  skillFile: string; // relative to skills/
  recipe: ContextRecipe;
  intervalMs: number;
}

export class TiandaoAgent {
  readonly name: string;
  private systemPrompt: string;
  private recipe: ContextRecipe;
  private lastRunTs = 0;
  readonly intervalMs: number;

  constructor(private config: AgentConfig) {
    this.name = config.name;
    this.recipe = config.recipe;
    this.intervalMs = config.intervalMs;
    this.systemPrompt = readFileSync(
      resolve(__dirname, "skills", config.skillFile),
      "utf-8",
    );
  }

  shouldRun(now: number): boolean {
    return now - this.lastRunTs >= this.intervalMs;
  }

  async tick(
    client: OpenAI,
    model: string,
    state: WorldStateV1,
  ): Promise<AgentDecision | null> {
    const now = Date.now();
    if (!this.shouldRun(now)) return null;

    this.lastRunTs = now;

    const context = assembleContext(this.recipe, state);
    const userPrompt = `${context}\n\n---\n\n请基于以上信息决策。输出 JSON。如果不需要行动，返回空数组。`;

    console.log(`[tiandao][${this.name}] thinking...`);

    const raw = await chat(client, model, [
      { role: "system", content: this.systemPrompt },
      { role: "user", content: userPrompt },
    ]);

    console.log(`[tiandao][${this.name}] response:\n${raw}\n`);

    const decision = parseDecision(raw);
    decision.commands.forEach((cmd) => {
      // Tag source for arbiter
      (cmd as Record<string, unknown>)._source = this.name;
    });

    return decision;
  }
}
