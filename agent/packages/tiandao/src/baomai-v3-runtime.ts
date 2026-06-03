import {
  CHANNELS,
  type BaomaiSkillEventV1,
  type BaomaiV3BloodBurnV1,
  type BaomaiV3MountainShakeV1,
  type BaomaiV3OverloadRippleV1,
  type BaomaiV3TranscendenceExpiredV1,
  type Narration,
  validateBaomaiSkillEventV1Contract,
  validateBaomaiV3BloodBurnV1,
  validateBaomaiV3MountainShakeV1,
  validateBaomaiV3OverloadRippleV1,
  validateBaomaiV3TranscendenceExpiredV1,
  validateNarrationV1Contract,
} from "@bong/schema";
import type { ValidationResult } from "@bong/schema";

const {
  AGENT_NARRATE,
  BAOMAI_V3_SKILL_EVENT,
  BAOMAI_V3_MOUNTAIN_SHAKE,
  BAOMAI_V3_BLOOD_BURN,
  BAOMAI_V3_TRANSCENDENCE_EXPIRED,
  BAOMAI_V3_OVERLOAD_RIPPLE,
} = CHANNELS;

export interface BaomaiV3RuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface BaomaiV3RuntimeStats {
  received: number;
  ignored: number;
  published: number;
  rejectedContract: number;
}

export interface BaomaiV3RuntimeConfig {
  sub: BaomaiV3RuntimeClient;
  pub: BaomaiV3RuntimeClient;
  logger?: Pick<typeof console, "info" | "warn">;
}

// ─── render functions ─────────────────────────────────────────────────────────

export function renderBaomaiV3Narration(event: BaomaiSkillEventV1): Narration | null {
  if (event.type !== "baomai_skill_event") return null;
  const actor = shortName(event.caster_id);
  const meridians = event.meridian_ids.length > 0 ? event.meridian_ids.join("、") : "经脉";
  let text: string;
  switch (event.skill_id) {
    case "beng_quan":
      text = `${actor} 贴身把真元压进拳骨，${meridians} 上裂纹一闪，劲力沉到对手胸前。`;
      break;
    case "full_power_charge":
      text = `${actor} 收肩沉息，整池真元向拳锋聚拢，手三阳先响了一声闷雷。`;
      break;
    case "full_power_release":
      text = `${actor} 把蓄满的一拳递出去，真元没有绕路，整条脉线只剩一记直撞。`;
      break;
    case "mountain_shake":
    case "blood_burn":
      // 专用富事件（BAOMAI_V3_MOUNTAIN_SHAKE / BAOMAI_V3_BLOOD_BURN channel）
      // 已承担唯一叙事来源；skill_event channel 对这两招不叙事，防止双重叙事 UX 回归。
      return null;
    case "disperse":
      text = event.flow_rate_multiplier >= 10
        ? `${actor} 烧去半池真元重铸凡躯，五息之内脉流暴涨十倍，却没有一分免伤余地。`
        : `${actor} 强行散功，凡躯没有应声，只白白折去一截真元池。`;
      break;
    default:
      text = `${actor} 运起爆脉法，肉身里传出一声低闷脉响。`;
      break;
  }

  const narration: Narration = {
    scope: "player",
    target: event.caster_id,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

/** 山震 AoE 叙事：命中数 + 震波强度 */
export function renderMountainShakeNarration(event: BaomaiV3MountainShakeV1): Narration | null {
  const actor = shortName(event.caster_id);
  let text: string;
  if (event.affected_count === 0) {
    text = `${actor} 把真元灌进地层，震波滚涌出去，周围却无人被抬乱。`;
  } else if (event.affected_count === 1) {
    text = `${actor} 一拳砸地，震波直接掀翻了对面一人，脚步彻底失序。`;
  } else {
    text = `${actor} 运气山震，一记足以让方圆 ${event.radius_blocks.toFixed(0)} 尺内 ${event.affected_count} 人同时踉跄的冲击沿地面传开。`;
  }

  const narration: Narration = {
    scope: "player",
    target: event.caster_id,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

/** 血燃叙事：普通档 vs 近死分支差异化 */
export function renderBloodBurnNarration(event: BaomaiV3BloodBurnV1): Narration | null {
  const actor = shortName(event.caster_id);
  let text: string;
  if (event.ended_in_near_death) {
    text = `${actor} 以命换劲推到了极限，血线烧尽后落入濒死——那口猛劲最终把自己逼到了悬崖边。`;
  } else {
    text = `${actor} 割去一截血量，换来真元池短暂膨涨 ${event.qi_multiplier.toFixed(1)} 倍，热血腥气弥漫皮肤。`;
  }

  const narration: Narration = {
    scope: "player",
    target: event.caster_id,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

/** 超越到期叙事：涣散感 */
export function renderTranscendenceExpiredNarration(
  event: BaomaiV3TranscendenceExpiredV1,
): Narration | null {
  const actor = shortName(event.caster_id);
  const text = `${actor} 以散功换来的暴走窗口到头了——脉流骤然泄回凡躯，那股汹涌之势如潮水退去。`;

  const narration: Narration = {
    scope: "player",
    target: event.caster_id,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

/** 过载涟漪叙事：total_severity 危机感 3 档 */
export function renderOverloadRippleNarration(event: BaomaiV3OverloadRippleV1): Narration | null {
  const actor = shortName(event.caster_id);
  const meridians = event.meridian_ids.length > 0 ? event.meridian_ids.join("、") : "经脉";
  let text: string;
  if (event.total_severity >= 0.8) {
    // 危机档：脉道几乎崩断
    text = `${actor} 的 ${meridians} 已被反复过载撕出深痕，总损伤累积至 ${Math.round(event.total_severity * 100)}%——脉道边缘的裂纹已开始互相连通，距断脉只剩最后一步。`;
  } else if (event.total_severity >= 0.4) {
    // 警示档：中度损伤
    text = `${actor} 的 ${meridians} 裂痕持续扩展，本次过载再添 ${(event.severity_delta * 100).toFixed(1)}%，累计损伤 ${Math.round(event.total_severity * 100)}%，脉气已开始外溢。`;
  } else {
    // 轻微档
    text = `${actor} 的 ${meridians} 因过载震出一道细纹，损伤累积至 ${Math.round(event.total_severity * 100)}%，暂时尚在可控范围。`;
  }

  const narration: Narration = {
    scope: "player",
    target: event.caster_id,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

// ─── channel handler type ─────────────────────────────────────────────────────

interface ChannelHandler {
  validate(data: unknown): ValidationResult;
  render(data: unknown): Narration | null;
}

// ─── runtime ──────────────────────────────────────────────────────────────────

export class BaomaiV3NarrationRuntime {
  private readonly sub: BaomaiV3RuntimeClient;
  private readonly pub: BaomaiV3RuntimeClient;
  private readonly logger: Pick<typeof console, "info" | "warn">;
  private connected = false;

  readonly stats: BaomaiV3RuntimeStats = {
    received: 0,
    ignored: 0,
    published: 0,
    rejectedContract: 0,
  };

  /**
   * plan-combat-skill-feedback-bridges-v1 P2 — 多通道路由表
   * 每条 channel 对应独立的 validate + render handler。
   * 原 BAOMAI_V3_SKILL_EVENT 路径保留，向后兼容。
   */
  private readonly routeTable: Record<string, ChannelHandler> = {
    [BAOMAI_V3_SKILL_EVENT]: {
      validate: validateBaomaiSkillEventV1Contract,
      render: (data) => renderBaomaiV3Narration(data as BaomaiSkillEventV1),
    },
    [BAOMAI_V3_MOUNTAIN_SHAKE]: {
      validate: validateBaomaiV3MountainShakeV1,
      render: (data) => renderMountainShakeNarration(data as BaomaiV3MountainShakeV1),
    },
    [BAOMAI_V3_BLOOD_BURN]: {
      validate: validateBaomaiV3BloodBurnV1,
      render: (data) => renderBloodBurnNarration(data as BaomaiV3BloodBurnV1),
    },
    [BAOMAI_V3_TRANSCENDENCE_EXPIRED]: {
      validate: validateBaomaiV3TranscendenceExpiredV1,
      render: (data) => renderTranscendenceExpiredNarration(data as BaomaiV3TranscendenceExpiredV1),
    },
    [BAOMAI_V3_OVERLOAD_RIPPLE]: {
      validate: validateBaomaiV3OverloadRippleV1,
      render: (data) => renderOverloadRippleNarration(data as BaomaiV3OverloadRippleV1),
    },
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (!(channel in this.routeTable)) return;
    void this.handlePayload(channel, message);
  };

  constructor(config: BaomaiV3RuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    // Subscribe all channels in routeTable
    for (const channel of Object.keys(this.routeTable)) {
      await this.sub.subscribe(channel);
    }
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(
      `[baomai-v3-runtime] subscribed to ${Object.keys(this.routeTable).join(", ")}`,
    );
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(channelOrMessage: string, maybeMessage?: string): Promise<void> {
    // Support both (channel, message) and (message) calling conventions
    const [channel, message] =
      maybeMessage === undefined
        ? [BAOMAI_V3_SKILL_EVENT, channelOrMessage] // legacy single-arg fallback
        : [channelOrMessage, maybeMessage];

    const handler = this.routeTable[channel];
    if (!handler) {
      // Unknown channel — ignore silently (caller already filtered via onMessage)
      this.stats.ignored += 1;
      return;
    }

    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[baomai-v3-runtime] non-JSON payload:", error);
      return;
    }

    const validation = handler.validate(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[baomai-v3-runtime] invalid payload:", validation.errors);
      return;
    }
    this.stats.received += 1;

    const narration = handler.render(parsed);
    if (!narration) {
      this.stats.ignored += 1;
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[baomai-v3-runtime] publish failed:", error);
    }
  }
}

function shortName(id: string): string {
  const stripped = id.replace(/^offline:/u, "");
  return stripped.length > 0 ? stripped : "某体修";
}
