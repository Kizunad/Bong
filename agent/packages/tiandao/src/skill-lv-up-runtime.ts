import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type Narration,
  type SkillLvUpPayloadV1,
  validateSkillLvUpPayloadV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { SKILL_LV_UP, AGENT_NARRATE } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

export interface SkillLvUpNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error?: (...args: unknown[]) => void;
}

export interface SkillLvUpNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface SkillLvUpNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: SkillLvUpNarrationRuntimeClient;
  pub: SkillLvUpNarrationRuntimeClient;
  logger?: SkillLvUpNarrationRuntimeLogger;
  systemPrompt?: string;
}

export interface SkillLvUpNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  llmFailures: number;
  fallbackUsed: number;
}

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "skill-lv-up.md"), "utf-8");
}

function fallbackNarration(payload: SkillLvUpPayloadV1): Narration {
  const skillLabel = skillDisplayName(payload.skill);
  return {
    scope: "player",
    target: narrationTarget(payload),
    text: `你于${skillLabel}一道又进一层，今至${levelLabel(payload.new_lv)}，手法稍熟，心中却并无喜色。`,
    style: "narration",
  };
}

function narrationTarget(payload: SkillLvUpPayloadV1): string {
  return `char:${payload.char_id}|skill:${payload.skill}|lv:${payload.new_lv}`;
}

function skillDisplayName(skill: SkillLvUpPayloadV1["skill"]): string {
  switch (skill) {
    case "herbalism":
      return "采药";
    case "alchemy":
      return "炼丹";
    case "forging":
      return "锻造";
  }

  return "技艺";
}

function levelLabel(newLv: number): string {
  return `Lv.${newLv}`;
}

function parseNarrationContent(content: string, payload: SkillLvUpPayloadV1): Narration {
  const trimmed = content.trim();
  if (!trimmed) {
    return fallbackNarration(payload);
  }

  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed) ||
      typeof (parsed as { text?: unknown }).text !== "string" ||
      typeof (parsed as { style?: unknown }).style !== "string"
    ) {
      return fallbackNarration(payload);
    }

    const first = parsed as { text: string; style: Narration["style"] };
    return {
      scope: "player",
      target: narrationTarget(payload),
      text: first.text,
      style: first.style,
    };
  } catch {
    return fallbackNarration(payload);
  }
}

export class SkillLvUpNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: SkillLvUpNarrationRuntimeClient;
  private readonly pub: SkillLvUpNarrationRuntimeClient;
  private readonly logger: SkillLvUpNarrationRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: SkillLvUpNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== SKILL_LV_UP) return;
    void this.handlePayload(message);
  };

  constructor(config: SkillLvUpNarrationRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(SKILL_LV_UP);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[skill-lv-up-runtime] subscribed to ${SKILL_LV_UP}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[skill-lv-up-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateSkillLvUpPayloadV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[skill-lv-up-runtime] SkillLvUpPayloadV1 contract rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    const payload = parsed as SkillLvUpPayloadV1;
    this.stats.received += 1;

    let narration = fallbackNarration(payload);
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(payload) },
      ]);
      narration = parseNarrationContent(normalizeLlmChatResult(result, this.model).content, payload);
      if (narration.text === fallbackNarration(payload).text) {
        this.stats.fallbackUsed += 1;
      }
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      this.logger.warn("[skill-lv-up-runtime] LLM error:", error);
    }

    const envelope = {
      v: 1,
      narrations: [narration],
    };

    try {
      const subscribers = await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
      this.stats.published += 1;
      this.logger.info(
        `[skill-lv-up-runtime] published narration for char:${payload.char_id} lv=${payload.new_lv} (${subscribers} subs)`,
      );
    } catch (error) {
      this.logger.warn("[skill-lv-up-runtime] publish failed:", error);
    }
  }
}
