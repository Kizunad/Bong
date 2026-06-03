/**
 * plan-combat-skill-feedback-bridges-v1 P1 — 爆脉 v4 叙事 runtime。
 *
 * 订阅 `bong:baomai_v4/*`（psubscribe pattern），按 channel 后缀路由到各 render 函数：
 *   - scar_circuit_formed  → renderScarCircuitFormedNarration
 *   - scar_circuit_broken  → renderScarCircuitBrokenNarration
 *   - iron_cocoon_stage_up → renderIronCocoonStageUpNarration
 *   - resonance_lock       → renderResonanceLockNarration
 *   - resonance_lock_end   → renderResonanceLockEndNarration
 *
 * 设计为多通道路由表（避免 baomai_v3 单通道坑）。
 * crack_reading 走 S2C CustomPayload，agent 无需订阅。
 */

import {
  CHANNELS,
  type BaomaiV4IronCocoonStageUpV1,
  type BaomaiV4ResonanceLockEndV1,
  type BaomaiV4ResonanceLockV1,
  type BaomaiV4ScarCircuitBrokenV1,
  type BaomaiV4ScarCircuitFormedV1,
  type Narration,
  validateBaomaiV4IronCocoonStageUpV1,
  validateBaomaiV4ResonanceLockEndV1,
  validateBaomaiV4ResonanceLockV1,
  validateBaomaiV4ScarCircuitBrokenV1,
  validateBaomaiV4ScarCircuitFormedV1,
  validateNarrationV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE } = CHANNELS;
const BAOMAI_V4_PATTERN = "bong:baomai_v4/*";

// ── client interface ──────────────────────────────────────────────────────────

export interface BaomaiV4RuntimeClient {
  psubscribe(pattern: string): Promise<unknown>;
  on(event: string, listener: (pattern: string, channel: string, message: string) => void): unknown;
  off?(event: string, listener: (pattern: string, channel: string, message: string) => void): unknown;
  punsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

// ── stats ─────────────────────────────────────────────────────────────────────

export interface BaomaiV4RuntimeStats {
  received: number;
  ignored: number;
  published: number;
  rejectedContract: number;
}

// ── config ────────────────────────────────────────────────────────────────────

export interface BaomaiV4RuntimeConfig {
  sub: BaomaiV4RuntimeClient;
  pub: BaomaiV4RuntimeClient;
  logger?: Pick<typeof console, "info" | "warn">;
}

// ── narration render functions ────────────────────────────────────────────────

function shortName(id: string): string {
  const stripped = id.replace(/^offline:/u, "");
  return stripped.length > 0 ? stripped : "某体修";
}

export function renderScarCircuitFormedNarration(event: BaomaiV4ScarCircuitFormedV1): Narration | null {
  const actor = shortName(event.entity_id);
  const circuitName = scarCircuitCN(event.circuit);
  const text = `${actor} 体内两条受损经脉的裂纹在${circuitName}处贯通，形成微弱次级通路——真元回流时带着一丝异样的嗡鸣。`;
  const narration: Narration = {
    scope: "player",
    target: event.entity_id,
    text,
    style: "perception",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export function renderScarCircuitBrokenNarration(event: BaomaiV4ScarCircuitBrokenV1): Narration | null {
  const actor = shortName(event.entity_id);
  const circuitName = scarCircuitCN(event.circuit);
  const reasonText = circuitBreakReasonCN(event.reason);
  const text = `${actor} 的${circuitName}次级通路${reasonText}，嗡鸣戛然而止。`;
  const narration: Narration = {
    scope: "player",
    target: event.entity_id,
    text,
    style: "perception",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export function renderIronCocoonStageUpNarration(event: BaomaiV4IronCocoonStageUpV1): Narration | null {
  const actor = shortName(event.entity_id);
  const fromName = ironCocoonStageCN(event.from);
  const toName = ironCocoonStageCN(event.to);
  const text = `${actor} 的肉身历经 ${event.total_overloads} 次过载，活茧由${fromName}蜕变为${toName}，组织里隐约传来真元矿化的低沉回声。`;
  const narration: Narration = {
    scope: "player",
    target: event.entity_id,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export function renderResonanceLockNarration(event: BaomaiV4ResonanceLockV1): Narration | null {
  const a = shortName(event.fighter_a_id);
  const b = shortName(event.fighter_b_id);
  const duration = event.ends_at - event.started_at;
  const text = `${a} 与 ${b} 的裂纹经脉在接触中共振锁定——双方真元振幅骤然放大，接下来 ${duration} 息是双向互噬的最险时刻。`;
  const narration: Narration = {
    scope: "broadcast",
    target: undefined,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export function renderResonanceLockEndNarration(event: BaomaiV4ResonanceLockEndV1): Narration | null {
  const a = shortName(event.fighter_a_id);
  const b = shortName(event.fighter_b_id);
  let text: string;
  switch (event.reason.type) {
    case "expired":
      text = `${a} 与 ${b} 的共振锁定自然消散，双方从互噬的临界边缘各自退回。`;
      break;
    case "retreated": {
      const retreater = shortName(event.reason.entity_id);
      text = `${retreater} 强行拉开距离，打破了共振锁定，互噬停止。`;
      break;
    }
    case "death": {
      const fallen = shortName(event.reason.entity_id);
      text = `${fallen} 在共振锁定中倒下，双向互噬随之终止。`;
      break;
    }
    default:
      return null;
  }
  const narration: Narration = {
    scope: "broadcast",
    target: undefined,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

// ── enum → CN ─────────────────────────────────────────────────────────────────

function scarCircuitCN(circuit: string): string {
  const map: Record<string, string> = {
    tiger_mouth: "虎口回路",
    triple_yang: "三阳合流",
    heart_lung: "心肺短路",
    liver_kidney: "肝肾交汇",
    gall_stomach: "胆胃通路",
    spleen_kidney: "脾肾固本",
    ren_du_bridge: "任督桥",
  };
  return map[circuit] ?? circuit;
}

function circuitBreakReasonCN(reason: string): string {
  const map: Record<string, string> = {
    healed: "因经脉修复而断开",
    deepened: "因损伤加深而断开",
    severed: "因经脉永久断脉而崩散",
    closed: "因经脉关闭而中断",
  };
  return map[reason] ?? "断开";
}

function ironCocoonStageCN(stage: string): string {
  const map: Record<string, string> = {
    none: "凡身",
    tough_skin: "茧皮",
    iron_bone: "茧骨",
    dull_flesh: "茧肉",
    scar_forged: "茧灵",
  };
  return map[stage] ?? stage;
}

// ── channel routing table ─────────────────────────────────────────────────────

type RouteTable = Record<string, {
  validate: (data: unknown) => { ok: boolean };
  render: (data: unknown) => Narration | null;
}>;

function buildRouteTable(): RouteTable {
  return {
    [CHANNELS.BAOMAI_V4_SCAR_CIRCUIT_FORMED]: {
      validate: validateBaomaiV4ScarCircuitFormedV1,
      render: (d) => renderScarCircuitFormedNarration(d as BaomaiV4ScarCircuitFormedV1),
    },
    [CHANNELS.BAOMAI_V4_SCAR_CIRCUIT_BROKEN]: {
      validate: validateBaomaiV4ScarCircuitBrokenV1,
      render: (d) => renderScarCircuitBrokenNarration(d as BaomaiV4ScarCircuitBrokenV1),
    },
    [CHANNELS.BAOMAI_V4_IRON_COCOON_STAGE_UP]: {
      validate: validateBaomaiV4IronCocoonStageUpV1,
      render: (d) => renderIronCocoonStageUpNarration(d as BaomaiV4IronCocoonStageUpV1),
    },
    [CHANNELS.BAOMAI_V4_RESONANCE_LOCK]: {
      validate: validateBaomaiV4ResonanceLockV1,
      render: (d) => renderResonanceLockNarration(d as BaomaiV4ResonanceLockV1),
    },
    [CHANNELS.BAOMAI_V4_RESONANCE_LOCK_END]: {
      validate: validateBaomaiV4ResonanceLockEndV1,
      render: (d) => renderResonanceLockEndNarration(d as BaomaiV4ResonanceLockEndV1),
    },
  };
}

// ── BaomaiV4NarrationRuntime class ───────────────────────────────────────────

export class BaomaiV4NarrationRuntime {
  private readonly sub: BaomaiV4RuntimeClient;
  private readonly pub: BaomaiV4RuntimeClient;
  private readonly logger: Pick<typeof console, "info" | "warn">;
  private readonly routeTable: RouteTable = buildRouteTable();
  private connected = false;

  readonly stats: BaomaiV4RuntimeStats = {
    received: 0,
    ignored: 0,
    published: 0,
    rejectedContract: 0,
  };

  private readonly onPMessage = (pattern: string, channel: string, message: string): void => {
    void this.handleMessage(channel, message);
  };

  constructor(config: BaomaiV4RuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.psubscribe(BAOMAI_V4_PATTERN);
    this.sub.off?.("pmessage", this.onPMessage);
    this.sub.on("pmessage", this.onPMessage);
    this.connected = true;
    this.logger.info(
      `[baomai-v4-runtime] psubscribed to ${BAOMAI_V4_PATTERN}`,
    );
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("pmessage", this.onPMessage);
    await this.sub.punsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handleMessage(channel: string, message: string): Promise<void> {
    const route = this.routeTable[channel];
    if (!route) {
      // unknown sub-channel — ignore silently (future-proof)
      this.stats.ignored += 1;
      return;
    }

    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        `[baomai-v4-runtime] non-JSON payload on ${channel}:`,
        error,
      );
      return;
    }

    const validation = route.validate(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        `[baomai-v4-runtime] invalid payload on ${channel}:`,
        validation,
      );
      return;
    }

    this.stats.received += 1;

    const narration = route.render(parsed);
    if (!narration) {
      this.stats.ignored += 1;
      return;
    }

    const envelope = { v: 1, narrations: [narration] };
    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[baomai-v4-runtime] publish failed:", error);
    }
  }
}
