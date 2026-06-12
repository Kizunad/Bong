/**
 * plan-halfstep-rechallenge-integration-v1 P1 — HalfStepRechallengeNarrationRuntime
 *
 * 订阅 `bong:tribulation/halfstep_rechallenge` Redis 频道，生成两类确定性叙事：
 * - player scope：「你感到曾遭封压的经脉微微松动，或许时机已到。」（target = char_id）
 * - zone scope（仅 zone_halfstep_count >= 2）：
 *   「虚空中某处的修士收到了相同的消息。」（target = zone_name）
 *
 * 使用确定性模板（不调用 LLM），参考 ElderEncounterNarrationRuntime 模式。
 */

import {
  CHANNELS,
  type HalfStepRechallengeTriggerPayloadV1,
  type Narration,
  validateHalfStepRechallengeTriggerPayloadV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, HALFSTEP_RECHALLENGE } = CHANNELS;

// ── 接口定义 ────────────────────────────────────────────────────────────────

export interface HalfStepRechallengeNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface HalfStepRechallengeNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface HalfStepRechallengeNarrationRuntimeConfig {
  sub: HalfStepRechallengeNarrationRuntimeClient;
  pub: HalfStepRechallengeNarrationRuntimeClient;
  logger?: HalfStepRechallengeNarrationRuntimeLogger;
}

export interface HalfStepRechallengeNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  ignored: number;
}

// ── Runtime 类 ──────────────────────────────────────────────────────────────

export class HalfStepRechallengeNarrationRuntime {
  private readonly sub: HalfStepRechallengeNarrationRuntimeClient;
  private readonly pub: HalfStepRechallengeNarrationRuntimeClient;
  private readonly logger: HalfStepRechallengeNarrationRuntimeLogger;
  private connected = false;

  readonly stats: HalfStepRechallengeNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    ignored: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== HALFSTEP_RECHALLENGE) return;
    void this.handlePayload(message);
  };

  constructor(config: HalfStepRechallengeNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(HALFSTEP_RECHALLENGE);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[halfstep-rechallenge-runtime] subscribed to ${HALFSTEP_RECHALLENGE}`);
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
      this.logger.warn("[halfstep-rechallenge-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateHalfStepRechallengeTriggerPayloadV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[halfstep-rechallenge-runtime] HalfStepRechallengeTriggerPayloadV1 contract rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    const payload = parsed as HalfStepRechallengeTriggerPayloadV1;
    const narrations = this.renderNarrations(payload);
    if (narrations.length === 0) {
      this.stats.ignored += 1;
      return;
    }

    this.stats.received += 1;

    const envelope = { v: 1, narrations };
    const envValidation = validateNarrationV1Contract(envelope);
    if (!envValidation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[halfstep-rechallenge-runtime] NarrationV1 contract rejected:",
        envValidation.errors.join("; "),
      );
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[halfstep-rechallenge-runtime] publish failed:", error);
    }
  }

  private renderNarrations(payload: HalfStepRechallengeTriggerPayloadV1): Narration[] {
    const narrations: Narration[] = [];

    // player scope（常规）：通知当事修士重渡窗口开启
    narrations.push(renderPlayerNarration(payload));

    // zone scope（仅 zone_halfstep_count >= 2）：同 zone 多名半步修士共振叙事
    if (payload.zone_halfstep_count >= 2) {
      narrations.push(renderZoneEchoNarration(payload));
    }

    return narrations;
  }
}

// ── 叙事模板 ────────────────────────────────────────────────────────────────

/**
 * player scope narration：通知触发方（玩家/NPC char_id）重渡窗口已开启。
 * style = "perception"：修士感知到自身经脉的微妙变化。
 */
export function renderPlayerNarration(
  payload: HalfStepRechallengeTriggerPayloadV1,
): Narration {
  return {
    scope: "player",
    target: payload.char_id,
    text: "你感到曾遭封压的经脉微微松动，或许时机已到。",
    style: "perception",
  };
}

/**
 * zone scope narration（仅 zone_halfstep_count >= 2）：
 * 同 zone 存在多名半步修士，虚空中回响着相似的真元波动。
 * style = "perception"：隐约感应，非直接告知。
 */
export function renderZoneEchoNarration(
  payload: HalfStepRechallengeTriggerPayloadV1,
): Narration {
  return {
    scope: "zone",
    target: payload.zone_name,
    text: "虚空中某处的修士收到了相同的消息。",
    style: "perception",
  };
}
