/**
 * 天道 Agent — 单个 Agent 实例
 * 加载 skill prompt + 拼装上下文 + 调用 LLM + 解析决策
 */

import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type { WorldStateV1 } from "@bong/schema";
import type { ChatSignal } from "@bong/schema";
import { type ContextRecipe, assembleContext, createContextInput } from "./context.js";
import { normalizeLlmChatResult, type LlmClient, type LlmToolUsage } from "./llm.js";
import { type AgentDecision, parseDecision } from "./parse.js";
import { createToolContext, type AgentTool } from "./tools/types.js";
import { queryPlayerTool } from "./tools/query-player.js";
import { queryPlayerSkillMilestonesTool } from "./tools/query-player-skill-milestones.js";
import { queryZoneHistoryTool } from "./tools/query-zone-history.js";
import { listActiveEventsTool } from "./tools/list-active-events.js";
import type { WorldModel } from "./world-model.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

export function resolveAgentTools(skillFile: string): readonly AgentTool[] {
  switch (skillFile) {
    case "calamity.md":
      return [queryPlayerTool, queryPlayerSkillMilestonesTool, listActiveEventsTool];
    case "mutation.md":
      return [queryZoneHistoryTool];
    case "era.md":
      return [];
    default:
      return [];
  }
}

export interface AgentConfig {
  name: string;
  skillFile: string; // relative to skills/
  recipe: ContextRecipe;
  intervalMs: number;
  model?: string;
  now?: () => number;
  tools?: readonly AgentTool[];
}

export interface AgentTickMetadata {
  durationMs: number;
  tokensEstimated: number;
  model: string;
  toolUsage?: LlmToolUsage;
}

export type AgentDecisionWithMetadata = AgentDecision & {
  __agentTickMetadata?: AgentTickMetadata;
};

export class TiandaoAgent {
  readonly name: string;
  readonly model?: string;
  private systemPrompt: string;
  private recipe: ContextRecipe;
  private lastRunTs = 0;
  readonly intervalMs: number;
  private latestChatSignals: ChatSignal[] = [];
  private worldModel?: WorldModel;
  private readonly now: () => number;
  private readonly tools: readonly AgentTool[];

  constructor(config: AgentConfig) {
    this.name = config.name;
    this.model = config.model;
    this.recipe = config.recipe;
    this.intervalMs = config.intervalMs;
    this.now = config.now ?? (() => Date.now());
    this.tools = config.tools ?? resolveAgentTools(config.skillFile);
    this.systemPrompt = readFileSync(
      resolve(__dirname, "skills", config.skillFile),
      "utf-8",
    );
  }

  setChatSignals(signals: ChatSignal[]): void {
    this.latestChatSignals = signals;
  }

  setWorldModel(worldModel: WorldModel): void {
    this.worldModel = worldModel;
  }

  shouldRun(now: number): boolean {
    return now - this.lastRunTs >= this.intervalMs;
  }

  async tick(
    client: LlmClient,
    model: string,
    state: WorldStateV1,
  ): Promise<AgentDecisionWithMetadata | null> {
    const now = this.now();
    if (!this.shouldRun(now)) return null;

    this.lastRunTs = now;

    const nowSeconds = Math.floor(now / 1000);
    const context = assembleContext(
      this.recipe,
      createContextInput(state, this.latestChatSignals, nowSeconds, {
        agentName: this.name,
        worldModel: this.worldModel,
      }),
    );
    const userPrompt = `${context}\n\n---\n\n请基于以上信息决策。输出 JSON。如果不需要行动，返回空数组。`;
    const effectiveModel = this.model ?? model;
    const chatOptions =
      this.tools.length > 0
        ? {
            tools: this.tools,
            toolContext: createToolContext({
              latestState: state,
              worldModel: this.worldModel,
            }),
          }
        : undefined;

    console.log(`[tiandao][${this.name}] thinking...`);

    const messages = [
      { role: "system", content: this.systemPrompt },
      { role: "user", content: userPrompt },
    ] satisfies Parameters<LlmClient["chat"]>[1];
    const rawResult = chatOptions
      ? await client.chat(effectiveModel, messages, chatOptions)
      : await client.chat(effectiveModel, messages);
    const result = normalizeLlmChatResult(rawResult, effectiveModel);

    console.log(`[tiandao][${this.name}] response:\n${result.content}\n`);

    const decision = parseDecision(result.content) as AgentDecisionWithMetadata;
    decision.__agentTickMetadata = {
      durationMs: result.durationMs,
      tokensEstimated:
        estimateTokens(this.systemPrompt) +
        estimateTokens(userPrompt) +
        estimateTokens(result.content),
      model: result.model,
      toolUsage: result.toolUsage,
    };
    return decision;
  }
}

function estimateTokens(text: string): number {
  return Math.max(0, Math.ceil(text.length / 4));
}
