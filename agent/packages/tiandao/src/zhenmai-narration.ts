import {
  CHANNELS,
  type CombatRealtimeEventV1,
  type Narration,
  validateCombatRealtimeEventV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

const { COMBAT_REALTIME, AGENT_NARRATE } = CHANNELS;

export interface ZhenmaiNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface ZhenmaiNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface ZhenmaiNarrationRuntimeStats {
  received: number;
  ignored: number;
  published: number;
  rejectedContract: number;
}

export interface ZhenmaiNarrationRuntimeConfig {
  sub: ZhenmaiNarrationRuntimeClient;
  pub: ZhenmaiNarrationRuntimeClient;
  logger?: ZhenmaiNarrationRuntimeLogger;
}

export function renderZhenmaiNarration(event: CombatRealtimeEventV1): Narration | null {
  if (event.kind !== "combat_event" || event.defense_kind !== "jie_mai") return null;
  const effectiveness = clampNumber(event.defense_effectiveness, 0.3, 1.0);
  const target = `zhenmai:parry|target:${event.target_id}|tick:${event.tick}`;
  const actor = shortName(event.target_id);
  let text: string;
  if (effectiveness >= 0.7) {
    text = `${actor} 皮下真元一震，异音未及入脉便被截断，千钧之势只剩半息僵直。`;
  } else if (effectiveness > 0.3) {
    text = `${actor} 勉强引爆截脉，外劲被削去一截，身形却已露出破绽。`;
  } else {
    text = `${actor} 被逼到贴身处才震爆，经脉护住了些，反冲却全压回血肉里。`;
  }

  const narration: Narration = {
    scope: "player",
    target,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export class ZhenmaiNarrationRuntime {
  private readonly sub: ZhenmaiNarrationRuntimeClient;
  private readonly pub: ZhenmaiNarrationRuntimeClient;
  private readonly logger: ZhenmaiNarrationRuntimeLogger;
  private connected = false;

  readonly stats: ZhenmaiNarrationRuntimeStats = {
    received: 0,
    ignored: 0,
    published: 0,
    rejectedContract: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== COMBAT_REALTIME) return;
    void this.handlePayload(message);
  };

  constructor(config: ZhenmaiNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(COMBAT_REALTIME);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[zhenmai-runtime] subscribed to ${COMBAT_REALTIME}`);
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
      this.logger.warn("[zhenmai-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateCombatRealtimeEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[zhenmai-runtime] invalid combat event:", validation.errors.join("; "));
      return;
    }
    this.stats.received += 1;

    const narration = renderZhenmaiNarration(parsed as CombatRealtimeEventV1);
    if (!narration) {
      this.stats.ignored += 1;
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[zhenmai-runtime] publish failed:", error);
    }
  }
}

function clampNumber(value: unknown, min: number, max: number): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.max(min, Math.min(max, value))
    : min;
}

function shortName(id: string): string {
  const stripped = id.replace(/^offline:/u, "");
  return stripped.length > 0 ? stripped : "某修士";
}
