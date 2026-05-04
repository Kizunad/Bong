import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type Narration,
  type ShedEventV1,
  validateNarrationV1Contract,
  validateShedEventV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { TUIKE_SHED, AGENT_NARRATE } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

export interface TuikeNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface TuikeNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface TuikeNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: TuikeNarrationRuntimeClient;
  pub: TuikeNarrationRuntimeClient;
  logger?: TuikeNarrationRuntimeLogger;
  systemPrompt?: string;
}

export interface TuikeNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  llmFailures: number;
  fallbackUsed: number;
}

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "tuike.md"), "utf-8");
}

function kindLabel(kind: ShedEventV1["kind"]): string {
  return kind === "rotten_wood_armor" ? "朽木甲" : "蛛丝伪皮";
}

function fallbackNarration(payload: ShedEventV1): Narration {
  const attacker = payload.attacker_id ? `被 ${payload.attacker_id} 逼得` : "受击后";
  const overflow = payload.contam_overflow > 0
    ? `，余下 ${payload.contam_overflow.toFixed(1)} 点污染仍钻进真身`
    : "";
  return {
    scope: "broadcast",
    target: `tuike:shed|target:${payload.target_id}|tick:${payload.tick}`,
    text: `${payload.target_id} ${attacker}蜕下 ${payload.layers_shed} 层${kindLabel(payload.kind)}，连同 ${payload.contam_absorbed.toFixed(1)} 点异种真元一并弃去${overflow}。`,
    style: "narration",
  };
}

function parseNarrationContent(content: string, payload: ShedEventV1): Narration {
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

export class TuikeNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: TuikeNarrationRuntimeClient;
  private readonly pub: TuikeNarrationRuntimeClient;
  private readonly logger: TuikeNarrationRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: TuikeNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== TUIKE_SHED) return;
    void this.handlePayload(message);
  };

  constructor(config: TuikeNarrationRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(TUIKE_SHED);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[tuike-runtime] subscribed to ${TUIKE_SHED}`);
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
      this.logger.warn("[tuike-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateShedEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      return;
    }
    const payload = parsed as ShedEventV1;
    this.stats.received += 1;

    let narration = fallbackNarration(payload);
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(payload) },
      ]);
      narration = parseNarrationContent(normalizeLlmChatResult(result, this.model).content, payload);
      if (narration.text === fallbackNarration(payload).text) this.stats.fallbackUsed += 1;
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      this.logger.warn("[tuike-runtime] LLM error:", error);
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[tuike-runtime] publish failed:", error);
    }
  }
}
