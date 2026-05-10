import {
  CHANNELS,
  type DuguReverseTriggeredV1,
  type DuguSelfCureProgressV1,
  type DuguV2SkillCastV1,
  type Narration,
  validateDuguReverseTriggeredV1Contract,
  validateDuguSelfCureProgressV1Contract,
  validateDuguV2SkillCastV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { AGENT_NARRATE, DUGU_V2_CAST, DUGU_V2_REVERSE, DUGU_V2_SELF_CURE } = CHANNELS;

export interface DuguV2RuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface DuguV2RuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: DuguV2RuntimeClient;
  pub: DuguV2RuntimeClient;
  logger?: Pick<Console, "info" | "warn">;
}

type DuguV2Payload =
  | { kind: "cast"; payload: DuguV2SkillCastV1 }
  | { kind: "self_cure"; payload: DuguSelfCureProgressV1 }
  | { kind: "reverse"; payload: DuguReverseTriggeredV1 };

const SKILL_LABEL: Record<DuguV2SkillCastV1["skill"], string> = {
  eclipse: "蚀针",
  self_cure: "自蕴",
  penetrate: "侵染",
  shroud: "神识遮蔽",
  reverse: "倒蚀",
};

export function renderDuguV2Narration(event: DuguV2Payload): Narration {
  if (event.kind === "self_cure") {
    const p = event.payload;
    const revealed = p.self_revealed ? "，形貌已遮不住阴诡色" : "";
    return {
      scope: "broadcast",
      target: `dugu_v2:self_cure|caster:${p.caster}|tick:${p.tick}`,
      text: `${p.caster} 以毒汤淬脉，阴诡色涨至 ${p.insidious_color_percent.toFixed(1)}%${revealed}。`,
      style: "narration",
    };
  }
  if (event.kind === "reverse") {
    const p = event.payload;
    const juebi = p.juebi_delay_ticks ? "，天道排异的风已经落下" : "";
    return {
      scope: "broadcast",
      target: `dugu_v2:reverse|caster:${p.caster}|tick:${p.tick}`,
      text: `${p.caster} 远指倒蚀，${p.affected_targets} 道旧毒同刻翻身，经脉内里裂响${juebi}。`,
      style: "narration",
    };
  }
  const p = event.payload;
  const target = p.target ? `，指向 ${p.target}` : "";
  const taint = p.taint_tier === "permanent" ? "，毒入髓中不退" : p.taint_tier === "temporary" ? "，蛊毒入脉" : "";
  return {
    scope: "broadcast",
    target: `dugu_v2:cast|caster:${p.caster}|skill:${p.skill}|tick:${p.tick}`,
    text: `${p.caster} 起${SKILL_LABEL[p.skill]}${target}${taint}。`,
    style: "narration",
  };
}

function parseNarration(content: string, fallback: Narration): Narration {
  try {
    const parsed = JSON.parse(content.trim()) as unknown;
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return fallback;
    const candidate = parsed as { text?: unknown; style?: unknown };
    if (typeof candidate.text !== "string" || typeof candidate.style !== "string") return fallback;
    const narration: Narration = {
      scope: fallback.scope,
      target: fallback.target,
      text: candidate.text,
      style: candidate.style as Narration["style"],
    };
    return validateNarrationV1Contract({ v: 1, narrations: [narration] }).ok
      ? narration
      : fallback;
  } catch {
    return fallback;
  }
}

export class DuguV2NarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: DuguV2RuntimeClient;
  private readonly pub: DuguV2RuntimeClient;
  private readonly logger: Pick<Console, "info" | "warn">;

  readonly stats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    void this.handlePayload(channel, message);
  };

  constructor(config: DuguV2RuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    await this.sub.subscribe(DUGU_V2_CAST);
    await this.sub.subscribe(DUGU_V2_SELF_CURE);
    await this.sub.subscribe(DUGU_V2_REVERSE);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.logger.info("[dugu-v2-runtime] subscribed");
  }

  async disconnect(): Promise<void> {
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
      this.logger.warn("[dugu-v2-runtime] non-JSON payload:", error);
      return;
    }
    const event = parseEvent(channel, parsed);
    if (!event) {
      this.stats.rejectedContract += 1;
      return;
    }
    this.stats.received += 1;

    const fallback = renderDuguV2Narration(event);
    let narration = fallback;
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: "按末法残土叙事口吻，用一条 JSON {\"text\",\"style\"} 描述毒蛊五招，不解释机制。" },
        { role: "user", content: JSON.stringify(event) },
      ]);
      narration = parseNarration(normalizeLlmChatResult(result, this.model).content, fallback);
      if (narration.text === fallback.text) this.stats.fallbackUsed += 1;
    } catch {
      this.stats.fallbackUsed += 1;
    }

    await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
    this.stats.published += 1;
  }
}

function parseEvent(channel: string, parsed: unknown): DuguV2Payload | null {
  if (channel === DUGU_V2_CAST) {
    return validateDuguV2SkillCastV1Contract(parsed).ok
      ? { kind: "cast", payload: parsed as DuguV2SkillCastV1 }
      : null;
  }
  if (channel === DUGU_V2_SELF_CURE) {
    return validateDuguSelfCureProgressV1Contract(parsed).ok
      ? { kind: "self_cure", payload: parsed as DuguSelfCureProgressV1 }
      : null;
  }
  if (channel === DUGU_V2_REVERSE) {
    return validateDuguReverseTriggeredV1Contract(parsed).ok
      ? { kind: "reverse", payload: parsed as DuguReverseTriggeredV1 }
      : null;
  }
  return null;
}
