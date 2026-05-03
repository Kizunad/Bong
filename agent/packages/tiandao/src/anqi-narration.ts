import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type CarrierImpactEventV1,
  type Narration,
  type ProjectileDespawnedEventV1,
  validateCarrierImpactEventV1Contract,
  validateNarrationV1Contract,
  validateProjectileDespawnedEventV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { ANQI_CARRIER_IMPACT, ANQI_PROJECTILE_DESPAWNED, AGENT_NARRATE } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

export interface AnqiNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface AnqiNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: AnqiNarrationRuntimeClient;
  pub: AnqiNarrationRuntimeClient;
  logger?: Pick<Console, "info" | "warn">;
  systemPrompt?: string;
}

type AnqiPayload =
  | { kind: "impact"; payload: CarrierImpactEventV1 }
  | { kind: "despawned"; payload: ProjectileDespawnedEventV1 };

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "anqi.md"), "utf-8");
}

function fallbackNarration(event: AnqiPayload): Narration {
  if (event.kind === "impact") {
    const payload = event.payload;
    return {
      scope: "broadcast",
      target: `anqi:impact|attacker:${payload.attacker}|target:${payload.target}|tick:${payload.tick}`,
      text: `${payload.attacker} 在 ${payload.hit_distance.toFixed(0)} 格外碎骨注元，${payload.target} 胸口一滞，异色真元已入经脉。`,
      style: "narration",
    };
  }
  const payload = event.payload;
  const text =
    payload.residual_qi > 0
      ? `${payload.projectile} 射空落地，七成真元被天地吞去，只余一点冷光很快熄灭。`
      : `${payload.projectile} 离靶而去，封存真元散得干净。`;
  return {
    scope: "broadcast",
    target: `anqi:miss|projectile:${payload.projectile}|tick:${payload.tick}`,
    text,
    style: "narration",
  };
}

function parseNarrationContent(content: string, payload: AnqiPayload): Narration {
  const fallback = fallbackNarration(payload);
  const trimmed = content.trim();
  if (!trimmed) return fallback;
  try {
    const parsed = JSON.parse(trimmed) as { text?: unknown; style?: unknown };
    if (typeof parsed.text !== "string" || typeof parsed.style !== "string") return fallback;
    const narration: Narration = {
      scope: fallback.scope,
      target: fallback.target,
      text: parsed.text,
      style: parsed.style as Narration["style"],
    };
    return validateNarrationV1Contract({ v: 1, narrations: [narration] }).ok
      ? narration
      : fallback;
  } catch {
    return fallback;
  }
}

export class AnqiNarrationRuntime {
  private connected = false;
  private readonly logger: Pick<Console, "info" | "warn">;
  private readonly systemPrompt: string;

  readonly stats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== ANQI_CARRIER_IMPACT && channel !== ANQI_PROJECTILE_DESPAWNED) return;
    void this.handlePayload(channel, message);
  };

  constructor(private readonly config: AnqiNarrationRuntimeConfig) {
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.config.sub.subscribe(ANQI_CARRIER_IMPACT);
    await this.config.sub.subscribe(ANQI_PROJECTILE_DESPAWNED);
    this.config.sub.off?.("message", this.onMessage);
    this.config.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[anqi-runtime] subscribed to ${ANQI_CARRIER_IMPACT}, ${ANQI_PROJECTILE_DESPAWNED}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.config.sub.off?.("message", this.onMessage);
    await this.config.sub.unsubscribe();
    this.config.sub.disconnect();
    this.config.pub.disconnect();
  }

  async handlePayload(channel: string, message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[anqi-runtime] non-JSON payload:", error);
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
      const result = await this.config.llm.chat(this.config.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(event) },
      ]);
      narration = parseNarrationContent(
        normalizeLlmChatResult(result, this.config.model).content,
        event,
      );
      if (narration.text === fallbackNarration(event).text) this.stats.fallbackUsed += 1;
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      this.logger.warn("[anqi-runtime] LLM error:", error);
    }

    try {
      await this.config.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[anqi-runtime] publish failed:", error);
    }
  }
}

function parseEvent(channel: string, parsed: unknown): AnqiPayload | null {
  if (channel === ANQI_CARRIER_IMPACT) {
    const validation = validateCarrierImpactEventV1Contract(parsed);
    return validation.ok ? { kind: "impact", payload: parsed as CarrierImpactEventV1 } : null;
  }
  if (channel === ANQI_PROJECTILE_DESPAWNED) {
    const validation = validateProjectileDespawnedEventV1Contract(parsed);
    return validation.ok
      ? { kind: "despawned", payload: parsed as ProjectileDespawnedEventV1 }
      : null;
  }
  return null;
}
