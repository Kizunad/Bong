import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type AntidoteResultEventV1,
  type DuguPoisonProgressEventV1,
  type Narration,
  validateAntidoteResultEventV1Contract,
  validateDuguPoisonProgressEventV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { DUGU_POISON_PROGRESS, DUGU_ANTIDOTE_RESULT, AGENT_NARRATE } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

export interface DuguNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface DuguNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface DuguNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: DuguNarrationRuntimeClient;
  pub: DuguNarrationRuntimeClient;
  logger?: DuguNarrationRuntimeLogger;
  systemPrompt?: string;
}

export interface DuguNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  llmFailures: number;
  fallbackUsed: number;
}

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "dugu.md"), "utf-8");
}

function fallbackNarration(payload: DuguPoisonProgressEventV1): Narration {
  const loss = Math.max(0, payload.actual_loss_this_tick);
  const lossText = loss >= 1 ? loss.toFixed(1) : loss.toFixed(2);
  return {
    scope: "player",
    target: payload.target,
    text: `${payload.target} 的 ${payload.meridian_id} 又被毒蛊蚀去 ${lossText} 容量，真元上限余 ${payload.qi_max_after.toFixed(1)}。`,
    style: "narration",
  };
}

function fallbackNarrationAntidote(payload: AntidoteResultEventV1): Narration {
  const text =
    payload.result === "success"
      ? `${payload.healer} 为 ${payload.target} 解除了 ${payload.meridian_id} 的毒蛊侵蚀，真元上限回升至 ${payload.qi_max_after.toFixed(1)}。`
      : `${payload.healer} 尝试为 ${payload.target} 净化 ${payload.meridian_id}，然而蛊毒根深，解毒失败，真元上限仍余 ${payload.qi_max_after.toFixed(1)}。`;
  return {
    scope: "player",
    target: payload.target,
    text,
    style: "narration",
  };
}

function parseNarrationContent(content: string, payload: DuguPoisonProgressEventV1): Narration {
  const trimmed = content.trim();
  if (!trimmed) return fallbackNarration(payload);
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
    const fallback = fallbackNarration(payload);
    const narration: Narration = {
      scope: fallback.scope,
      target: fallback.target,
      text: first.text,
      style: first.style,
    };
    const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
    return validation.ok ? narration : fallback;
  } catch {
    return fallbackNarration(payload);
  }
}

function parseNarrationContentAntidote(
  content: string,
  payload: AntidoteResultEventV1,
): Narration {
  const trimmed = content.trim();
  if (!trimmed) return fallbackNarrationAntidote(payload);
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed) ||
      typeof (parsed as { text?: unknown }).text !== "string" ||
      typeof (parsed as { style?: unknown }).style !== "string"
    ) {
      return fallbackNarrationAntidote(payload);
    }
    const first = parsed as { text: string; style: Narration["style"] };
    const fallback = fallbackNarrationAntidote(payload);
    const narration: Narration = {
      scope: fallback.scope,
      target: fallback.target,
      text: first.text,
      style: first.style,
    };
    const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
    return validation.ok ? narration : fallback;
  } catch {
    return fallbackNarrationAntidote(payload);
  }
}

export class DuguNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: DuguNarrationRuntimeClient;
  private readonly pub: DuguNarrationRuntimeClient;
  private readonly logger: DuguNarrationRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: DuguNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== DUGU_POISON_PROGRESS && channel !== DUGU_ANTIDOTE_RESULT) return;
    void this.handlePayload(channel, message);
  };

  constructor(config: DuguNarrationRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(DUGU_POISON_PROGRESS);
    await this.sub.subscribe(DUGU_ANTIDOTE_RESULT);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[dugu-runtime] subscribed to ${DUGU_POISON_PROGRESS}`);
    this.logger.info(`[dugu-runtime] subscribed to ${DUGU_ANTIDOTE_RESULT}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(channel: string, message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[dugu-runtime] non-JSON payload:", error);
      return;
    }

    if (channel === DUGU_ANTIDOTE_RESULT) {
      await this.handleAntidoteResult(parsed);
      return;
    }

    // DUGU_POISON_PROGRESS path (unchanged)
    const validation = validateDuguPoisonProgressEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      return;
    }
    const payload = parsed as DuguPoisonProgressEventV1;
    this.stats.received += 1;

    let narration = fallbackNarration(payload);
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(payload) },
      ]);
      narration = parseNarrationContent(
        normalizeLlmChatResult(result, this.model).content,
        payload,
      );
      if (narration.text === fallbackNarration(payload).text) this.stats.fallbackUsed += 1;
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      this.logger.warn("[dugu-runtime] LLM error:", error);
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[dugu-runtime] publish failed:", error);
    }
  }

  private async handleAntidoteResult(parsed: unknown): Promise<void> {
    const validation = validateAntidoteResultEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      return;
    }
    const payload = parsed as AntidoteResultEventV1;
    this.stats.received += 1;

    let narration = fallbackNarrationAntidote(payload);
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(payload) },
      ]);
      narration = parseNarrationContentAntidote(
        normalizeLlmChatResult(result, this.model).content,
        payload,
      );
      if (narration.text === fallbackNarrationAntidote(payload).text) this.stats.fallbackUsed += 1;
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      this.logger.warn("[dugu-runtime] LLM error (antidote):", error);
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[dugu-runtime] publish failed (antidote):", error);
    }
  }
}
