import {
  CHANNELS,
  type Narration,
  type TuikeV2SkillEventV1,
  validateNarrationV1Contract,
  validateTuikeV2SkillEventV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { AGENT_NARRATE, TUIKE_V2_SKILL_EVENT } = CHANNELS;

export interface TuikeV2RuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface TuikeV2RuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: TuikeV2RuntimeClient;
  pub: TuikeV2RuntimeClient;
  logger?: Pick<Console, "info" | "warn">;
}

const SKILL_LABEL: Record<TuikeV2SkillEventV1["skill_id"], string> = {
  don: "着壳",
  shed: "蜕一层",
  transfer_taint: "转移污染",
};

const TIER_LABEL: Record<TuikeV2SkillEventV1["tier"], string> = {
  fan: "凡级伪皮",
  light: "轻档伪皮",
  mid: "中档伪皮",
  heavy: "重档伪皮",
  ancient: "上古级伪皮",
};

export function renderTuikeV2Narration(payload: TuikeV2SkillEventV1): Narration {
  if (payload.skill_id === "don") {
    return {
      scope: "broadcast",
      target: `tuike_v2:don|caster:${payload.caster_id}|tick:${payload.tick}`,
      text: `${payload.caster_id} 披上${TIER_LABEL[payload.tier]}，外息贴着皮面浮起，只多一层钱买来的命。`,
      style: "narration",
    };
  }
  if (payload.skill_id === "shed") {
    const ancient = payload.tier === "ancient" ? "上古皮一脆，光从裂处透出来，" : "";
    const damage = payload.damage_absorbed
      ? `挡下 ${payload.damage_absorbed.toFixed(1)} 伤，`
      : "";
    const permanent = payload.permanent_absorbed > 0
      ? `连 ${payload.permanent_absorbed.toFixed(2)} 成永久衰败标记也一并带走，`
      : "";
    return {
      scope: "broadcast",
      target: `tuike_v2:shed|caster:${payload.caster_id}|tick:${payload.tick}`,
      text: `${payload.caster_id} 蜕下${TIER_LABEL[payload.tier]}，${ancient}${damage}${permanent}地上只剩一张冷皮。`,
      style: "narration",
    };
  }
  const permanent = payload.permanent_absorbed > 0
    ? `，毒蛊永久标记被按进皮里 ${payload.permanent_absorbed.toFixed(2)} 成`
    : "";
  return {
    scope: "broadcast",
    target: `tuike_v2:transfer|caster:${payload.caster_id}|tick:${payload.tick}`,
    text: `${payload.caster_id} 指尖一压经脉，把 ${payload.contam_moved_percent.toFixed(1)}% 污染推到${TIER_LABEL[payload.tier]}${permanent}。`,
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

export class TuikeV2NarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: TuikeV2RuntimeClient;
  private readonly pub: TuikeV2RuntimeClient;
  private readonly logger: Pick<Console, "info" | "warn">;

  readonly stats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== TUIKE_V2_SKILL_EVENT) return;
    this.handlePayload(message).catch((error) => {
      this.logger.warn("[tuike-v2-runtime] failed to handle payload:", error);
    });
  };

  constructor(config: TuikeV2RuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    await this.sub.subscribe(TUIKE_V2_SKILL_EVENT);
    this.logger.info("[tuike-v2-runtime] subscribed");
  }

  async disconnect(): Promise<void> {
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
      this.logger.warn("[tuike-v2-runtime] non-JSON payload:", error);
      return;
    }
    const validation = validateTuikeV2SkillEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      return;
    }
    const payload = parsed as TuikeV2SkillEventV1;
    this.stats.received += 1;

    const fallback = renderTuikeV2Narration(payload);
    let narration = fallback;
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: "按末法残土叙事口吻，用一条 JSON {\"text\",\"style\"} 描述替尸蜕壳三招，不解释机制。" },
        { role: "user", content: JSON.stringify(payload) },
      ]);
      narration = parseNarration(normalizeLlmChatResult(result, this.model).content, fallback);
      if (narration.text === fallback.text) this.stats.fallbackUsed += 1;
    } catch (error) {
      this.stats.fallbackUsed += 1;
      this.logger.warn("[tuike-v2-runtime] narration fallback used:", error);
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[tuike-v2-runtime] failed to publish narration:", error);
    }
  }
}
