import {
  CHANNELS,
  type BreakthroughCinematicEventV1,
  type Narration,
  validateBreakthroughCinematicEventV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { AGENT_NARRATE, BREAKTHROUGH_CINEMATIC } = CHANNELS;

export interface BreakthroughCinematicNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error?: (...args: unknown[]) => void;
}

export interface BreakthroughCinematicNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface BreakthroughCinematicNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: BreakthroughCinematicNarrationRuntimeClient;
  pub: BreakthroughCinematicNarrationRuntimeClient;
  logger?: BreakthroughCinematicNarrationRuntimeLogger;
  systemPrompt?: string;
}

export interface BreakthroughCinematicNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  llmFailures: number;
  fallbackUsed: number;
}

const DEFAULT_PROMPT =
  "你是 Bong 的天道旁白。根据突破 cinematic phase 输出一条短中文 narration，必须冷淡、具体、不写 UI、不编造 payload 外事实。只输出 JSON：{\"text\":\"...\",\"style\":\"narration\"}。";

export function fallbackBreakthroughCinematicNarration(
  payload: BreakthroughCinematicEventV1,
): Narration {
  const target = narrationTarget(payload);
  const realm = realmLabel(payload.realm_to);
  let text: string;
  switch (payload.phase) {
    case "prelude":
      text = "此处灵气开始聚拢。";
      break;
    case "charge":
      text = isHighRealm(payload) ? "天道注意到了。" : "一缕灵气被牵进丹田，尚算安分。";
      break;
    case "catalyze":
      text = `${payload.actor_id} 身侧灵压成环，光柱将起。`;
      break;
    case "apex":
      text = payload.result === "success"
        ? `${payload.actor_id} 叩开${realm}，天地只亮了一息。`
        : "灵机散了。天地并不回头。";
      break;
    case "aftermath":
      text = payload.interrupted
        ? `${payload.actor_id} 破境被截，余光碎在地上。`
        : payload.result === "success"
          ? `${realm}已成，余韵渐灭。`
          : "不过如此。";
      break;
  }
  return {
    scope: payload.visible_radius_blocks >= 5000 ? "broadcast" : "zone",
    target,
    text,
    style: "narration",
  };
}

function narrationTarget(payload: BreakthroughCinematicEventV1): string {
  return `breakthrough:${payload.actor_id}|${payload.phase}|${payload.realm_from}->${payload.realm_to}`;
}

function isHighRealm(payload: BreakthroughCinematicEventV1): boolean {
  return payload.realm_to === "Spirit" || payload.realm_to === "Void";
}

function realmLabel(realm: BreakthroughCinematicEventV1["realm_to"]): string {
  switch (realm) {
    case "Induce":
      return "引气";
    case "Condense":
      return "凝脉";
    case "Solidify":
      return "固元";
    case "Spirit":
      return "通灵";
    case "Void":
      return "化虚";
    default:
      return "新境";
  }
}

function parseNarrationContent(content: string, payload: BreakthroughCinematicEventV1): Narration {
  const fallback = fallbackBreakthroughCinematicNarration(payload);
  const trimmed = content.trim();
  if (!trimmed) return fallback;
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed) ||
      typeof (parsed as { text?: unknown }).text !== "string" ||
      typeof (parsed as { style?: unknown }).style !== "string"
    ) {
      return fallback;
    }
    const candidate = parsed as { text: string; style: Narration["style"] };
    const narration: Narration = {
      scope: fallback.scope,
      target: fallback.target,
      text: candidate.text,
      style: candidate.style,
    };
    const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
    return validation.ok ? narration : fallback;
  } catch {
    return fallback;
  }
}

export class BreakthroughCinematicNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: BreakthroughCinematicNarrationRuntimeClient;
  private readonly pub: BreakthroughCinematicNarrationRuntimeClient;
  private readonly logger: BreakthroughCinematicNarrationRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: BreakthroughCinematicNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== BREAKTHROUGH_CINEMATIC) return;
    void this.handlePayload(message);
  };

  constructor(config: BreakthroughCinematicNarrationRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? DEFAULT_PROMPT;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(BREAKTHROUGH_CINEMATIC);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[breakthrough-cinematic-runtime] subscribed to ${BREAKTHROUGH_CINEMATIC}`);
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
      this.logger.warn("[breakthrough-cinematic-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateBreakthroughCinematicEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[breakthrough-cinematic-runtime] BreakthroughCinematicEventV1 rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    const payload = parsed as BreakthroughCinematicEventV1;
    this.stats.received += 1;
    const fallback = fallbackBreakthroughCinematicNarration(payload);
    let narration = fallback;

    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(payload) },
      ]);
      narration = parseNarrationContent(normalizeLlmChatResult(result, this.model).content, payload);
      if (narration.text === fallback.text) {
        this.stats.fallbackUsed += 1;
      }
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      narration = fallback;
      this.logger.warn("[breakthrough-cinematic-runtime] LLM error:", error);
    }

    await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
    this.stats.published += 1;
  }
}
