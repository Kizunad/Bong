import {
  CHANNELS,
  type Narration,
  type VoidActionBroadcastV1,
  validateNarrationV1Contract,
  validateVoidActionBroadcastV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const {
  AGENT_NARRATE,
  VOID_ACTION_SUPPRESS_TSY,
  VOID_ACTION_EXPLODE_ZONE,
  VOID_ACTION_BARRIER,
  VOID_ACTION_LEGACY_ASSIGN,
} = CHANNELS;

const VOID_ACTION_CHANNELS = [
  VOID_ACTION_SUPPRESS_TSY,
  VOID_ACTION_EXPLODE_ZONE,
  VOID_ACTION_BARRIER,
  VOID_ACTION_LEGACY_ASSIGN,
] as const;

export interface VoidActionNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface VoidActionNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface VoidActionNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: VoidActionNarrationRuntimeClient;
  pub: VoidActionNarrationRuntimeClient;
  logger?: VoidActionNarrationRuntimeLogger;
  systemPrompt?: string;
}

export interface VoidActionNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  llmFailures: number;
  fallbackUsed: number;
}

function defaultSystemPrompt(): string {
  return [
    "你是末法残土的天道旁白。",
    "根据化虚者 action payload 输出 JSON：{\"text\":\"...\",\"style\":\"narration\"}。",
    "语气冷静、古意、短句；只写公开可见结果，不泄露数值公式、冷却、内部字段或概率。",
  ].join("\n");
}

function narrationTarget(payload: VoidActionBroadcastV1): string {
  return `void_action:${payload.kind}|actor:${payload.actor_id}|target:${payload.target}|tick:${payload.at_tick}`;
}

function fallbackText(payload: VoidActionBroadcastV1): string {
  if (payload.public_text.trim()) {
    return payload.public_text;
  }
  switch (payload.kind) {
    case "suppress_tsy":
      return `${payload.actor_name} 镇住 ${payload.target}，坍缩渊暂退一线。`;
    case "explode_zone":
      return `${payload.actor_name} 引爆 ${payload.target}，灵机骤盛，六月后只余空壳。`;
    case "barrier":
      return `${payload.actor_name} 在 ${payload.target} 立下化虚障，道伥过线自折其气。`;
    case "legacy_assign":
      return `${payload.actor_name} 留下临终遗令，道统指向 ${payload.target}。`;
  }
}

function fallbackNarration(payload: VoidActionBroadcastV1): Narration {
  return {
    scope: "broadcast",
    target: narrationTarget(payload),
    text: fallbackText(payload),
    style: "narration",
  };
}

function parseNarrationContent(content: string, payload: VoidActionBroadcastV1): Narration {
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

    if (leaksInternalVoidActionDetails(trimmed)) {
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

function leaksInternalVoidActionDetails(content: string): boolean {
  return /qi_cost|lifespan_cost|cooldown|ready_at|ledger|WorldQiBudget|概率|百分比|%|tick|payload|JSON/i.test(
    content,
  );
}

function channelForPayload(payload: VoidActionBroadcastV1): string {
  switch (payload.kind) {
    case "suppress_tsy":
      return VOID_ACTION_SUPPRESS_TSY;
    case "explode_zone":
      return VOID_ACTION_EXPLODE_ZONE;
    case "barrier":
      return VOID_ACTION_BARRIER;
    case "legacy_assign":
      return VOID_ACTION_LEGACY_ASSIGN;
  }
}

export class VoidActionNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: VoidActionNarrationRuntimeClient;
  private readonly pub: VoidActionNarrationRuntimeClient;
  private readonly logger: VoidActionNarrationRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;
  private pending: Promise<void> = Promise.resolve();

  readonly stats: VoidActionNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (!VOID_ACTION_CHANNELS.includes(channel as (typeof VOID_ACTION_CHANNELS)[number])) return;
    this.pending = this.pending
      .then(() => this.handlePayload(channel, message))
      .catch((error: unknown) => {
        this.logger.warn("[void-actions-runtime] queued payload failed:", error);
      });
  };

  constructor(config: VoidActionNarrationRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    for (const channel of VOID_ACTION_CHANNELS) {
      await this.sub.subscribe(channel);
    }
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[void-actions-runtime] subscribed to ${VOID_ACTION_CHANNELS.join(", ")}`);
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
      this.logger.warn("[void-actions-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateVoidActionBroadcastV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[void-actions-runtime] VoidActionBroadcastV1 contract rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    const payload = parsed as VoidActionBroadcastV1;
    if (channelForPayload(payload) !== channel) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        `[void-actions-runtime] channel/kind mismatch channel=${channel} kind=${payload.kind}`,
      );
      return;
    }

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
      this.logger.warn("[void-actions-runtime] LLM error:", error);
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[void-actions-runtime] publish failed:", error);
    }
  }
}
