/**
 * plan-dying-elder-v1 P3 — ElderEncounterNarrationRuntime
 *
 * 订阅 `bong:elder_encounter` Redis 频道，按 event_kind 生成两类叙事：
 * - 感知类（zone scope）：`appeared`（大能出现）/ `dan_received`（收丹计数更新）
 * - 死亡广播类（broadcast scope）：`betrayal` / `dead_natural` / `dead_player_kill`
 *
 * 使用确定性模板（不调用 LLM），参考 ScatteredCultivatorNarrationRuntime 模式。
 */

import {
  CHANNELS,
  type ElderEncounterEventV1,
  type Narration,
  validateElderEncounterEventV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, ELDER_ENCOUNTER } = CHANNELS;

// ── 接口定义 ────────────────────────────────────────────────────────────────

export interface ElderEncounterNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface ElderEncounterNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface ElderEncounterNarrationRuntimeConfig {
  sub: ElderEncounterNarrationRuntimeClient;
  pub: ElderEncounterNarrationRuntimeClient;
  logger?: ElderEncounterNarrationRuntimeLogger;
}

export interface ElderEncounterNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  ignored: number;
}

// ── Runtime 类 ──────────────────────────────────────────────────────────────

export class ElderEncounterNarrationRuntime {
  private readonly sub: ElderEncounterNarrationRuntimeClient;
  private readonly pub: ElderEncounterNarrationRuntimeClient;
  private readonly logger: ElderEncounterNarrationRuntimeLogger;
  private connected = false;

  readonly stats: ElderEncounterNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    ignored: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== ELDER_ENCOUNTER) return;
    void this.handlePayload(message);
  };

  constructor(config: ElderEncounterNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(ELDER_ENCOUNTER);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[elder-encounter-runtime] subscribed to ${ELDER_ENCOUNTER}`);
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
      this.logger.warn("[elder-encounter-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateElderEncounterEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[elder-encounter-runtime] ElderEncounterEventV1 contract rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    const payload = parsed as ElderEncounterEventV1;
    const narration = this.renderNarration(payload);
    if (narration === null) {
      this.stats.ignored += 1;
      return;
    }

    this.stats.received += 1;

    const envelope = { v: 1, narrations: [narration] };
    const envValidation = validateNarrationV1Contract(envelope);
    if (!envValidation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[elder-encounter-runtime] NarrationV1 contract rejected:",
        envValidation.errors.join("; "),
      );
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[elder-encounter-runtime] publish failed:", error);
    }
  }

  private renderNarration(payload: ElderEncounterEventV1): Narration | null {
    switch (payload.event_kind) {
      case "appeared":
        return renderAppearedNarration(payload);
      case "dan_received":
        return renderDanReceivedNarration(payload);
      case "betrayal":
        return renderDeathBroadcast(payload, "betrayal");
      case "dead_natural":
        return renderDeathBroadcast(payload, "dead_natural");
      case "dead_player_kill":
        return renderDeathBroadcast(payload, "dead_player_kill");
      default:
        return null;
    }
  }
}

// ── 叙事模板（感知类，zone scope） ─────────────────────────────────────────

/**
 * Appeared 叙事：大能出现在坍缩渊。
 * 2 种模板按 zone_name 哈希选择（稳定性）。
 */
export function renderAppearedNarration(payload: ElderEncounterEventV1): Narration {
  const text = appearedText(payload);
  return {
    scope: "zone",
    target: payload.zone_name,
    text,
    style: "narration",
    kind: "dying_elder_appeared",
  };
}

function appearedText(payload: ElderEncounterEventV1): string {
  // 用 zone_name 哈希选择模板（0 or 1），保证同 zone 每次叙事一致
  const variant = zoneNameHash(payload.zone_name) % 2;
  const dangerHint = payload.betray_probability > 0.7 ? "气息中暗含险机" : "深处真元有所动";

  if (variant === 0) {
    return `${payload.zone_name} 坍缩渊深处，一股垂死的化虚真元勉力维系。${dangerHint}，有修士求援之意。`;
  } else {
    return `${payload.zone_name} 虚裂之边，一名力竭大能压住最后一口元气，低声乞求过路者施以援手。${dangerHint}。`;
  }
}

/**
 * DanReceived 叙事：玩家给丹，计数更新。
 * 2 种模板按 elder_entity_id 哈希选择。
 */
export function renderDanReceivedNarration(payload: ElderEncounterEventV1): Narration {
  const text = danReceivedText(payload);
  return {
    scope: "zone",
    target: payload.zone_name,
    text,
    style: "narration",
    kind: "dying_elder_dan_received",
  };
}

function danReceivedText(payload: ElderEncounterEventV1): string {
  const variant = payload.elder_entity_id % 2;
  const progress = `${payload.dan_count}/5`;

  if (variant === 0) {
    return `大能吞下第 ${payload.dan_count} 颗回元丹（${progress}），真元稍稳，话语中感激与算计并存。`;
  } else {
    return `一颗回元丹渡入坍缩渊（已收 ${progress}）。大能气息微振，低语："尚差几颗……"`;
  }
}

// ── 叙事模板（死亡广播类，broadcast scope） ────────────────────────────────

/**
 * 死亡广播叙事（betrayal / dead_natural / dead_player_kill）。
 * 2 种模板按 elder_entity_id 哈希选择。
 */
export function renderDeathBroadcast(
  payload: ElderEncounterEventV1,
  kind: "betrayal" | "dead_natural" | "dead_player_kill",
): Narration {
  const text = deathBroadcastText(payload, kind);
  const narrationKind =
    kind === "betrayal"
      ? "dying_elder_betrayal"
      : kind === "dead_natural"
        ? "dying_elder_dead_natural"
        : "dying_elder_dead_player_kill";

  return {
    scope: "broadcast",
    target: `zone:${payload.zone_name}|elder:${payload.elder_entity_id}`,
    text,
    style: "narration",
    kind: narrationKind,
  };
}

function deathBroadcastText(
  payload: ElderEncounterEventV1,
  kind: "betrayal" | "dead_natural" | "dead_player_kill",
): string {
  const variant = payload.elder_entity_id % 2;

  if (kind === "betrayal") {
    if (variant === 0) {
      return `${payload.zone_name} 坍缩渊传来一声沉响——大能翻脸，夺舍之力瞬间爆发，所谓传承不过是诱饵。`;
    } else {
      return `${payload.zone_name} 深处的求援声骤然变质，化虚大能翻脸夺舍，真元悉数敛入己身，再无声息。`;
    }
  }

  if (kind === "dead_natural") {
    if (variant === 0) {
      return `${payload.zone_name} 坍缩渊那一缕垂危的化虚真元终于散尽，大能力竭，灵息随坍缩渊归于虚无。`;
    } else {
      return `${payload.zone_name} 深处，那名乞援大能的气息彻底熄灭——真元耗尽，连同承诺的传承一并消散。`;
    }
  }

  // dead_player_kill
  if (variant === 0) {
    return `${payload.zone_name} 坍缩渊传来一声轰响，垂死大能被外力击杀，传承随散逸的真元一并葬入虚裂。`;
  } else {
    return `${payload.zone_name} 深处，大能求援的气息被一记外力斩断，余烟散去，坍缩渊暂时安静了。`;
  }
}

// ── 工具函数 ────────────────────────────────────────────────────────────────

/**
 * 简单字符串哈希（djb2 风格），用于模板选择。
 * 同输入恒定输出，保证叙事稳定性。
 */
function zoneNameHash(name: string): number {
  let hash = 5381;
  for (let i = 0; i < name.length; i++) {
    hash = ((hash << 5) + hash + name.charCodeAt(i)) >>> 0;
  }
  return hash;
}
