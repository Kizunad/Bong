import {
  CHANNELS,
  type Narration,
  type TurbulenceFieldV1,
  type WoliuBackfireV1,
  type WoliuSkillCastV1,
  validateNarrationV1Contract,
  validateTurbulenceFieldV1Contract,
  validateWoliuBackfireV1Contract,
  validateWoliuSkillCastV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const {
  AGENT_NARRATE,
  WOLIU_V2_BACKFIRE,
  WOLIU_V2_CAST,
  WOLIU_V2_TURBULENCE,
} = CHANNELS;

export interface WoliuV2RuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface WoliuV2RuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: WoliuV2RuntimeClient;
  pub: WoliuV2RuntimeClient;
  logger?: Pick<Console, "info" | "warn">;
}

type WoliuV2Payload =
  | { kind: "cast"; payload: WoliuSkillCastV1 }
  | { kind: "backfire"; payload: WoliuBackfireV1 }
  | { kind: "turbulence"; payload: TurbulenceFieldV1 };

const SKILL_LABEL: Record<WoliuSkillCastV1["skill"], string> = {
  hold: "持涡",
  burst: "瞬涡",
  mouth: "涡口",
  pull: "涡引",
  heart: "涡心",
};

export function renderWoliuV2Narration(event: WoliuV2Payload): Narration {
  if (event.kind === "backfire") {
    const p = event.payload;
    const level = p.level === "severed" ? "手经尽断" : p.level === "torn" ? "经脉撕开" : "经脉发冷";
    return {
      scope: "broadcast",
      target: `woliu_v2:backfire|caster:${p.caster}|skill:${p.skill}|tick:${p.tick}`,
      text: `${p.caster} 强催${SKILL_LABEL[p.skill]}，紊流倒卷，${level}。`,
      style: "narration",
    };
  }
  if (event.kind === "turbulence") {
    const p = event.payload;
    return {
      scope: "broadcast",
      target: `woliu_v2:turbulence|caster:${p.caster}|skill:${p.skill}|tick:${p.tick}`,
      text: `${p.caster} 身畔紊流铺开 ${p.radius.toFixed(1)} 格，灵气如被搅碎，旁人难以定息。`,
      style: "narration",
    };
  }
  const p = event.payload;
  return {
    scope: "broadcast",
    target: `woliu_v2:cast|caster:${p.caster}|skill:${p.skill}|tick:${p.tick}`,
    text: `${p.caster} 起${SKILL_LABEL[p.skill]}，九成九真元甩作乱流，仅一点回入掌心。`,
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

export class WoliuV2NarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: WoliuV2RuntimeClient;
  private readonly pub: WoliuV2RuntimeClient;
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

  constructor(config: WoliuV2RuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    await this.sub.subscribe(WOLIU_V2_CAST);
    await this.sub.subscribe(WOLIU_V2_BACKFIRE);
    await this.sub.subscribe(WOLIU_V2_TURBULENCE);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.logger.info("[woliu-v2-runtime] subscribed");
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
      this.logger.warn("[woliu-v2-runtime] non-JSON payload:", error);
      return;
    }
    const event = parseEvent(channel, parsed);
    if (!event) {
      this.stats.rejectedContract += 1;
      return;
    }
    this.stats.received += 1;

    const fallback = renderWoliuV2Narration(event);
    let narration = fallback;
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: "按末法残土叙事口吻，用一条 JSON {\"text\",\"style\"} 描述涡流五招，不解释机制。" },
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

function parseEvent(channel: string, parsed: unknown): WoliuV2Payload | null {
  if (channel === WOLIU_V2_CAST) {
    return validateWoliuSkillCastV1Contract(parsed).ok
      ? { kind: "cast", payload: parsed as WoliuSkillCastV1 }
      : null;
  }
  if (channel === WOLIU_V2_BACKFIRE) {
    return validateWoliuBackfireV1Contract(parsed).ok
      ? { kind: "backfire", payload: parsed as WoliuBackfireV1 }
      : null;
  }
  if (channel === WOLIU_V2_TURBULENCE) {
    return validateTurbulenceFieldV1Contract(parsed).ok
      ? { kind: "turbulence", payload: parsed as TurbulenceFieldV1 }
      : null;
  }
  return null;
}
