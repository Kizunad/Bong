/**
 * plan-combat-skill-feedback-bridges-v1 P6 — 蜕壳灰烬入包叙事 runtime。
 *
 * 订阅 `bong:tuike_v2/ash_decay`（`CHANNELS.TUIKE_V2_ASH_DECAY`），
 * 验证 `TuikeAshDecayV1` schema，
 * 调用 `renderTuikeAshDecayNarration` fallback 叙事 → publish 到 `AGENT_NARRATE`。
 *
 * 结构仿 `meridian-severed-runtime.ts`（class + connect/disconnect + stats）。
 */

import {
  CHANNELS,
  type TuikeAshDecayV1,
  validateTuikeAshDecayV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, TUIKE_V2_ASH_DECAY } = CHANNELS;

export interface TuikeAshRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface TuikeAshRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
}

export interface TuikeAshRuntimeConfig {
  sub: TuikeAshRuntimeClient;
  pub: TuikeAshRuntimeClient;
  logger?: Pick<typeof console, "info" | "warn">;
}

const TIER_LABEL: Record<TuikeAshDecayV1["tier"], string> = {
  fan: "凡级伪皮",
  light: "轻档伪皮",
  mid: "中档伪皮",
  heavy: "重档伪皮",
  ancient: "上古级伪皮",
};

/**
 * 灰烬叙事 fallback 文本（无需 LLM 的轻量叙事，末法残土口吻）。
 *
 * Ancient tier 有专属文案（上古遗物碎片），普通档以品级命名区分。
 */
export function renderTuikeAshDecayNarration(payload: TuikeAshDecayV1): string {
  if (payload.tier === "ancient") {
    return `${payload.owner_id} 回收了一枚上古遗物碎片——曾经的${TIER_LABEL[payload.tier]}已化为灰，只剩这点残骸证明它存在过。`;
  }
  return `${payload.owner_id} 捡起一撮灰，${TIER_LABEL[payload.tier]}的残骸就是这些。`;
}

export class TuikeAshDecayNarrationRuntime {
  private readonly sub: TuikeAshRuntimeClient;
  private readonly pub: TuikeAshRuntimeClient;
  private readonly logger: Pick<typeof console, "info" | "warn">;
  private connected = false;

  readonly stats: TuikeAshRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== TUIKE_V2_ASH_DECAY) return;
    void this.handlePayload(message);
  };

  constructor(config: TuikeAshRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(TUIKE_V2_ASH_DECAY);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[tuike-ash-runtime] subscribed to ${TUIKE_V2_ASH_DECAY}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  /**
   * handlePayload(message) — 内部 onMessage 路径。
   * handlePayload(channel, message) — 测试直接调用路径（与 meridian-severed-runtime 对齐）。
   */
  async handlePayload(channelOrMessage: string, maybeMessage?: string): Promise<void> {
    const message = maybeMessage === undefined ? channelOrMessage : maybeMessage;
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[tuike-ash-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateTuikeAshDecayV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[tuike-ash-runtime] schema validation failed:",
        validation.errors.join("; "),
      );
      return;
    }
    const payload = parsed as TuikeAshDecayV1;
    this.stats.received += 1;

    const text = renderTuikeAshDecayNarration(payload);
    const narration = {
      scope: "broadcast" as const,
      target: `tuike_ash:decay|owner:${payload.owner_id}|tick:${payload.tick}`,
      text,
      style: "narration" as const,
    };

    try {
      await this.pub.publish(
        AGENT_NARRATE,
        JSON.stringify({ v: 1, narrations: [narration] }),
      );
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[tuike-ash-runtime] publish failed:", error);
    }
  }
}
