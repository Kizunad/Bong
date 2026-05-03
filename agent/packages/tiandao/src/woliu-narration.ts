import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type Narration,
  type ProjectileQiDrainedEventV1,
  type VortexBackfireEventV1,
  validateNarrationV1Contract,
  validateProjectileQiDrainedEventV1Contract,
  validateVortexBackfireEventV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { WOLIU_BACKFIRE, WOLIU_PROJECTILE_DRAINED, AGENT_NARRATE } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

export interface WoliuNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface WoliuNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface WoliuNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: WoliuNarrationRuntimeClient;
  pub: WoliuNarrationRuntimeClient;
  logger?: WoliuNarrationRuntimeLogger;
  systemPrompt?: string;
}

export interface WoliuNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  llmFailures: number;
  fallbackUsed: number;
}

type WoliuPayload =
  | { kind: "backfire"; payload: VortexBackfireEventV1 }
  | { kind: "projectile_drained"; payload: ProjectileQiDrainedEventV1 };

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "woliu.md"), "utf-8");
}

function fallbackNarration(event: WoliuPayload): Narration {
  if (event.kind === "backfire") {
    const payload = event.payload;
    const text = payload.resisted
      ? `${payload.caster} 强压涡流反噬，丹力替他挡下一劫，手经仍在发冷。`
      : payload.cause === "env_qi_too_low"
        ? `${payload.caster} 在贫瘠之地强造涡流，差值不足，反吸自身。`
        : `${payload.caster} 的涡流维持过久，反噬倒卷，一根经脉就此永封。`;
    return {
      scope: "broadcast",
      target: `woliu:backfire|caster:${payload.caster}|tick:${payload.tick}`,
      text,
      style: "narration",
    };
  }

  const payload = event.payload;
  const heavyDrain = payload.delta >= 0.45 || payload.remaining_payload <= payload.drained_amount;
  return {
    scope: "broadcast",
    target: `woliu:drain|caster:${payload.field_caster}|projectile:${payload.projectile}|tick:${payload.tick}`,
    text: heavyDrain
      ? `${payload.projectile} 入了 ${payload.field_caster} 的涡流，真元被天地抽得干净，载体仍向前飞，却已失了锋意。`
      : `${payload.field_caster} 身周涡流一转，${payload.projectile} 的真元被削去一截。`,
    style: "narration",
  };
}

function parseNarrationContent(content: string, payload: WoliuPayload): Narration {
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

export class WoliuNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: WoliuNarrationRuntimeClient;
  private readonly pub: WoliuNarrationRuntimeClient;
  private readonly logger: WoliuNarrationRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: WoliuNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== WOLIU_BACKFIRE && channel !== WOLIU_PROJECTILE_DRAINED) return;
    void this.handlePayload(channel, message);
  };

  constructor(config: WoliuNarrationRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(WOLIU_BACKFIRE);
    await this.sub.subscribe(WOLIU_PROJECTILE_DRAINED);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[woliu-runtime] subscribed to ${WOLIU_BACKFIRE}, ${WOLIU_PROJECTILE_DRAINED}`);
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
      this.logger.warn("[woliu-runtime] non-JSON payload:", error);
      return;
    }

    const event = parseEvent(channel, parsed);
    if (!event) {
      this.stats.rejectedContract += 1;
      return;
    }
    this.stats.received += 1;

    let narration = fallbackNarration(event);
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(event) },
      ]);
      narration = parseNarrationContent(normalizeLlmChatResult(result, this.model).content, event);
      if (narration.text === fallbackNarration(event).text) this.stats.fallbackUsed += 1;
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      this.logger.warn("[woliu-runtime] LLM error:", error);
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[woliu-runtime] publish failed:", error);
    }
  }
}

function parseEvent(channel: string, parsed: unknown): WoliuPayload | null {
  if (channel === WOLIU_BACKFIRE) {
    const validation = validateVortexBackfireEventV1Contract(parsed);
    return validation.ok ? { kind: "backfire", payload: parsed as VortexBackfireEventV1 } : null;
  }
  if (channel === WOLIU_PROJECTILE_DRAINED) {
    const validation = validateProjectileQiDrainedEventV1Contract(parsed);
    return validation.ok
      ? { kind: "projectile_drained", payload: parsed as ProjectileQiDrainedEventV1 }
      : null;
  }
  return null;
}
