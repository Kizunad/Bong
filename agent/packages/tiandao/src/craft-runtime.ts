import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type CraftOutcomeV1,
  type Narration,
  type RecipeUnlockedV1,
  validateCraftOutcomeV1Contract,
  validateNarrationV1Contract,
  validateRecipeUnlockedV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { CRAFT_OUTCOME, CRAFT_RECIPE_UNLOCKED, AGENT_NARRATE } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

/**
 * plan-craft-v1 P3 — agent 端 craft narration runtime。
 *
 * 订阅 2 条 Redis channel（`bong:craft/outcome` / `bong:craft/recipe_unlocked`），
 * 转换成 4 类叙事并通过 `bong:agent_narrate` 推回 server。4 类内部分类规则：
 *
 *  | 类别  | 触发                                        | 叙事重点                       |
 *  |-------|---------------------------------------------|--------------------------------|
 *  | 首学  | recipe_unlocked.source.kind=scroll          | 私自欢喜（残卷领悟）           |
 *  | 师承  | recipe_unlocked.source.kind=mentor          | 口传 / 手势 / 心法             |
 *  | 顿悟  | recipe_unlocked.source.kind=insight         | 心头一震（按 trigger 具象化）   |
 *  | 出炉  | craft_outcome.kind=completed                | 产物质感（不写数字）           |
 *
 * 失败 outcome（kind=failed）当前不生成叙事——客户端已有 toast，agent 静默。
 */
export interface CraftNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface CraftNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface CraftNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: CraftNarrationRuntimeClient;
  pub: CraftNarrationRuntimeClient;
  logger?: CraftNarrationRuntimeLogger;
  systemPrompt?: string;
}

export interface CraftNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  llmFailures: number;
  fallbackUsed: number;
  byCategory: {
    firstLearn: number;
    mentor: number;
    insight: number;
    completed: number;
  };
}

export type CraftNarrationCategory = "first_learn" | "mentor" | "insight" | "completed";

interface CraftNarrationInput {
  category: CraftNarrationCategory;
  recipeId: string;
  payload: RecipeUnlockedV1 | CraftOutcomeV1;
}

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "craft.md"), "utf-8");
}

/** 从 RecipeUnlockedV1 推导 narration category。 */
function categoryFromUnlock(payload: RecipeUnlockedV1): CraftNarrationCategory {
  switch (payload.source.kind) {
    case "scroll":
      return "first_learn";
    case "mentor":
      return "mentor";
    case "insight":
      return "insight";
  }
}

function fallbackForCategory(input: CraftNarrationInput): Narration {
  const targetPlayer = "player_id" in input.payload ? input.payload.player_id : "";
  switch (input.category) {
    case "first_learn":
      return {
        scope: "player",
        target: targetPlayer,
        text: `${targetPlayer} 灯下展残卷，对着字脚比划半夜，原来 ${input.recipeId} 是这般做的。`,
        style: "narration",
      };
    case "mentor":
      return {
        scope: "player",
        target: targetPlayer,
        text: `${targetPlayer} 听罢长辈那一段口诀手势，记进了里。`,
        style: "narration",
      };
    case "insight": {
      const trigger =
        input.payload && "source" in input.payload && input.payload.source.kind === "insight"
          ? input.payload.source.trigger
          : "";
      const triggerText = (() => {
        switch (trigger) {
          case "breakthrough":
            return "突破真元一震，丹田鸣响";
          case "near_death":
            return "血光退到指尖";
          case "defeat_stronger":
            return "刀光顿挫，敌人倒下";
          default:
            return "心头猝然一震";
        }
      })();
      return {
        scope: "player",
        target: targetPlayer,
        text: `${targetPlayer} ${triggerText}，那一线开窍便记下了。`,
        style: "narration",
      };
    }
    case "completed": {
      if (input.payload && "kind" in input.payload && input.payload.kind === "completed") {
        const output = input.payload.output_template;
        return {
          scope: "player",
          target: targetPlayer,
          text: `${targetPlayer} 收手时 ${output} 已成形，气息合在掌心。`,
          style: "narration",
        };
      }
      return {
        scope: "player",
        target: targetPlayer,
        text: `${targetPlayer} 收手了，物件成形。`,
        style: "narration",
      };
    }
  }
}

function parseNarrationContent(content: string, input: CraftNarrationInput): Narration {
  const trimmed = content.trim();
  if (!trimmed) return fallbackForCategory(input);
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed) ||
      typeof (parsed as { text?: unknown }).text !== "string" ||
      typeof (parsed as { style?: unknown }).style !== "string"
    ) {
      return fallbackForCategory(input);
    }
    const first = parsed as { text: string; style: Narration["style"] };
    const fallback = fallbackForCategory(input);
    const narration: Narration = {
      scope: fallback.scope,
      target: fallback.target,
      text: first.text,
      style: first.style,
    };
    const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
    return validation.ok ? narration : fallback;
  } catch {
    return fallbackForCategory(input);
  }
}

export class CraftNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: CraftNarrationRuntimeClient;
  private readonly pub: CraftNarrationRuntimeClient;
  private readonly logger: CraftNarrationRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: CraftNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    llmFailures: 0,
    fallbackUsed: 0,
    byCategory: { firstLearn: 0, mentor: 0, insight: 0, completed: 0 },
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel === CRAFT_OUTCOME) {
      void this.handleOutcome(message);
      return;
    }
    if (channel === CRAFT_RECIPE_UNLOCKED) {
      void this.handleUnlocked(message);
      return;
    }
  };

  constructor(config: CraftNarrationRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(CRAFT_OUTCOME);
    await this.sub.subscribe(CRAFT_RECIPE_UNLOCKED);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(
      `[craft-runtime] subscribed to ${CRAFT_OUTCOME} & ${CRAFT_RECIPE_UNLOCKED}`,
    );
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handleOutcome(message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[craft-runtime] outcome non-JSON:", error);
      return;
    }
    const validation = validateCraftOutcomeV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      return;
    }
    const payload = parsed as CraftOutcomeV1;
    if (payload.kind !== "completed") {
      // 失败 outcome 不生成 narration（client toast 已覆盖）
      return;
    }
    this.stats.received += 1;
    this.stats.byCategory.completed += 1;
    await this.emitNarration({
      category: "completed",
      recipeId: payload.recipe_id,
      payload,
    });
  }

  async handleUnlocked(message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[craft-runtime] unlocked non-JSON:", error);
      return;
    }
    const validation = validateRecipeUnlockedV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      return;
    }
    const payload = parsed as RecipeUnlockedV1;
    const category = categoryFromUnlock(payload);
    this.stats.received += 1;
    switch (category) {
      case "first_learn":
        this.stats.byCategory.firstLearn += 1;
        break;
      case "mentor":
        this.stats.byCategory.mentor += 1;
        break;
      case "insight":
        this.stats.byCategory.insight += 1;
        break;
    }
    await this.emitNarration({
      category,
      recipeId: payload.recipe_id,
      payload,
    });
  }

  private async emitNarration(input: CraftNarrationInput): Promise<void> {
    let narration = fallbackForCategory(input);
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        {
          role: "user",
          content: JSON.stringify({
            event:
              input.category === "completed" ? "craft_outcome" : "recipe_unlocked",
            category: input.category,
            data: input.payload,
          }),
        },
      ]);
      narration = parseNarrationContent(
        normalizeLlmChatResult(result, this.model).content,
        input,
      );
      if (narration.text === fallbackForCategory(input).text) this.stats.fallbackUsed += 1;
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      this.logger.warn("[craft-runtime] LLM error:", error);
    }
    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[craft-runtime] publish failed:", error);
    }
  }
}
