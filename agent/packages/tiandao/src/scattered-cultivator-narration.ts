import {
  CHANNELS,
  type Narration,
  type NicheIntrusionEventV1,
  validateNarrationV1Contract,
  validateNicheIntrusionEventV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, SOCIAL_NICHE_INTRUSION } = CHANNELS;

export interface ScatteredCultivatorNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface ScatteredCultivatorNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface ScatteredCultivatorNarrationRuntimeConfig {
  sub: ScatteredCultivatorNarrationRuntimeClient;
  pub: ScatteredCultivatorNarrationRuntimeClient;
  logger?: ScatteredCultivatorNarrationRuntimeLogger;
}

export interface ScatteredCultivatorNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  ignored: number;
}

export class ScatteredCultivatorNarrationRuntime {
  private readonly sub: ScatteredCultivatorNarrationRuntimeClient;
  private readonly pub: ScatteredCultivatorNarrationRuntimeClient;
  private readonly logger: ScatteredCultivatorNarrationRuntimeLogger;
  private connected = false;

  readonly stats: ScatteredCultivatorNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    ignored: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== SOCIAL_NICHE_INTRUSION) return;
    void this.handlePayload(channel, message);
  };

  constructor(config: ScatteredCultivatorNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(SOCIAL_NICHE_INTRUSION);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(
      `[scattered-cultivator-runtime] subscribed to ${SOCIAL_NICHE_INTRUSION}`,
    );
  }

  async disconnect(): Promise<void> {
    this.connected = false;
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
      this.logger.warn("[scattered-cultivator-runtime] non-JSON payload:", error);
      return;
    }

    const narration = this.parseNarration(channel, parsed);
    if (narration === null) return;

    const envelope = { v: 1, narrations: [narration] };
    const validation = validateNarrationV1Contract(envelope);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[scattered-cultivator-runtime] NarrationV1 contract rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[scattered-cultivator-runtime] publish failed:", error);
    }
  }

  private parseNarration(channel: string, parsed: unknown): Narration | null {
    if (channel === SOCIAL_NICHE_INTRUSION) {
      const validation = validateNicheIntrusionEventV1Contract(parsed);
      if (!validation.ok) {
        this.stats.rejectedContract += 1;
        this.logger.warn(
          "[scattered-cultivator-runtime] NicheIntrusionEventV1 contract rejected:",
          validation.errors.join("; "),
        );
        return null;
      }
      const payload = parsed as NicheIntrusionEventV1;
      if (!isNpcIntruder(payload.intruder_id)) {
        this.stats.ignored += 1;
        return null;
      }
      this.stats.received += 1;
      return renderNpcIntrusionNarration(payload);
    }

    this.stats.ignored += 1;
    return null;
  }
}

function isNpcIntruder(intruderId: string): boolean {
  return intruderId.startsWith("npc:") || intruderId.startsWith("npc_");
}

//TODO:lingtian_refactor 耕作叙事等待新版事件契约。

export function renderNpcIntrusionNarration(payload: NicheIntrusionEventV1): Narration {
  const [x, y, z] = payload.niche_pos;
  return {
    scope: "broadcast",
    target: `niche:${x},${y},${z}|intruder:${payload.intruder_id}`,
    text: `无灵龛的散修摸到 ${x},${y},${z}，取走 ${payload.items_taken.length} 件物，仍称只是借路。`,
    style: "narration",
    kind: "niche_intrusion_by_npc",
  };
}
