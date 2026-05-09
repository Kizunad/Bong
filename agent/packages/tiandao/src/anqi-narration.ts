import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type CarrierAbrasionEventV1,
  type CarrierImpactEventV1,
  type ContainerSwapEventV1,
  type EchoFractalEventV1,
  type MultiShotEventV1,
  type Narration,
  type ProjectileDespawnedEventV1,
  type QiInjectionEventV1,
  validateCarrierAbrasionEventV1Contract,
  validateCarrierImpactEventV1Contract,
  validateContainerSwapEventV1Contract,
  validateEchoFractalEventV1Contract,
  validateMultiShotEventV1Contract,
  validateNarrationV1Contract,
  validateProjectileDespawnedEventV1Contract,
  validateQiInjectionEventV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const {
  ANQI_CARRIER_IMPACT,
  ANQI_PROJECTILE_DESPAWNED,
  ANQI_MULTI_SHOT,
  ANQI_QI_INJECTION,
  ANQI_ECHO_FRACTAL,
  ANQI_CARRIER_ABRASION,
  ANQI_CONTAINER_SWAP,
  AGENT_NARRATE,
} = CHANNELS;
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
  | { kind: "despawned"; payload: ProjectileDespawnedEventV1 }
  | { kind: "multi_shot"; payload: MultiShotEventV1 }
  | { kind: "qi_injection"; payload: QiInjectionEventV1 }
  | { kind: "echo_fractal"; payload: EchoFractalEventV1 }
  | { kind: "abrasion"; payload: CarrierAbrasionEventV1 }
  | { kind: "container_swap"; payload: ContainerSwapEventV1 };

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
  if (event.kind === "multi_shot") {
    const payload = event.payload;
    return {
      scope: "broadcast",
      target: `anqi:multi_shot|caster:${payload.caster}|tick:${payload.tick}`,
      text: `${payload.caster} 抖开${payload.projectile_count}支灵木暗器，扇面收成 ${payload.cone_degrees.toFixed(0)} 度，冷光分路而去。`,
      style: "narration",
    };
  }
  if (event.kind === "qi_injection") {
    const payload = event.payload;
    const target = payload.target ?? "未知目标";
    const tear = payload.triggers_overload_tear ? "，经脉反噬已经记账" : "";
    return {
      scope: "broadcast",
      target: `anqi:inject|caster:${payload.caster}|target:${target}|skill:${payload.skill}|tick:${payload.tick}`,
      text: `${payload.caster} 以 ${payload.carrier_kind} 打出 ${payload.skill}，${target} 承了 ${payload.wound_qi.toFixed(0)} 点封元伤${tear}。`,
      style: "narration",
    };
  }
  if (event.kind === "echo_fractal") {
    const payload = event.payload;
    return {
      scope: "broadcast",
      target: `anqi:echo|caster:${payload.caster}|tick:${payload.tick}`,
      text: `${payload.caster} 把上古残骨压进化虚真元场，一支骨影裂成 ${payload.echo_count} 支 echo，逐支都能被拦截。`,
      style: "narration",
    };
  }
  if (event.kind === "abrasion") {
    const payload = event.payload;
    return {
      scope: "broadcast",
      target: `anqi:abrasion|carrier:${payload.carrier}|tick:${payload.tick}`,
      text: `${payload.container} ${payload.direction === "store" ? "入囊" : "出囊"}磨去 ${payload.lost_qi.toFixed(1)} 真元，载体余 ${payload.after_qi.toFixed(1)}。`,
      style: "system_warning",
    };
  }
  if (event.kind === "container_swap") {
    const payload = event.payload;
    return {
      scope: "broadcast",
      target: `anqi:container|carrier:${payload.carrier}|tick:${payload.tick}`,
      text: `${payload.carrier} 将暗器容器从 ${payload.from} 切到 ${payload.to}，暴露窗口持续到 ${payload.switching_until_tick} tick。`,
      style: "system_warning",
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
    if (!ANQI_CHANNELS.has(channel)) return;
    void this.handlePayload(channel, message);
  };

  constructor(private readonly config: AnqiNarrationRuntimeConfig) {
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    for (const channel of ANQI_CHANNELS) {
      await this.config.sub.subscribe(channel);
    }
    this.config.sub.off?.("message", this.onMessage);
    this.config.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[anqi-runtime] subscribed to ${[...ANQI_CHANNELS].join(", ")}`);
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
  if (channel === ANQI_MULTI_SHOT) {
    const validation = validateMultiShotEventV1Contract(parsed);
    return validation.ok ? { kind: "multi_shot", payload: parsed as MultiShotEventV1 } : null;
  }
  if (channel === ANQI_QI_INJECTION) {
    const validation = validateQiInjectionEventV1Contract(parsed);
    return validation.ok ? { kind: "qi_injection", payload: parsed as QiInjectionEventV1 } : null;
  }
  if (channel === ANQI_ECHO_FRACTAL) {
    const validation = validateEchoFractalEventV1Contract(parsed);
    return validation.ok ? { kind: "echo_fractal", payload: parsed as EchoFractalEventV1 } : null;
  }
  if (channel === ANQI_CARRIER_ABRASION) {
    const validation = validateCarrierAbrasionEventV1Contract(parsed);
    return validation.ok ? { kind: "abrasion", payload: parsed as CarrierAbrasionEventV1 } : null;
  }
  if (channel === ANQI_CONTAINER_SWAP) {
    const validation = validateContainerSwapEventV1Contract(parsed);
    return validation.ok ? { kind: "container_swap", payload: parsed as ContainerSwapEventV1 } : null;
  }
  return null;
}

const ANQI_CHANNELS = new Set<string>([
  ANQI_CARRIER_IMPACT,
  ANQI_PROJECTILE_DESPAWNED,
  ANQI_MULTI_SHOT,
  ANQI_QI_INJECTION,
  ANQI_ECHO_FRACTAL,
  ANQI_CARRIER_ABRASION,
  ANQI_CONTAINER_SWAP,
]);
