import type {
  Narration,
  PoisonDoseEventV1,
  PoisonOverdoseEventV1,
  PoisonSideEffectTagV1,
} from "@bong/schema";
import {
  CHANNELS,
  validateNarrationV1Contract,
  validatePoisonDoseEventV1,
  validatePoisonOverdoseEventV1,
} from "@bong/schema";

const { AGENT_NARRATE, POISON_DOSE_EVENT, POISON_OVERDOSE_EVENT } = CHANNELS;

const SIDE_EFFECT_TEXT: Record<PoisonSideEffectTagV1, string> = {
  qi_focus_drift_2h: "眼前准星微偏，真元像被一层灰雾拖住。",
  rage_burst_30min: "血气忽然上冲，力道涨了半分，步子却沉了一截。",
  hallucin_tint_6h: "视野边角泛起青鳞般的影，远近一时难辨。",
  digest_lock_6h: "丹毒锁在腹中不散，像一枚冷钉压着胃火。",
  toxicity_tier_unlock: "经络里有一线暗绿沉下去，附毒的门槛被硬生生推开。",
};

export function poisonSideEffectText(tag: PoisonSideEffectTagV1): string {
  return SIDE_EFFECT_TEXT[tag] ?? "丹毒沿经络一沉，气息一时发涩。";
}

export function renderPoisonDoseNarration(event: PoisonDoseEventV1): Narration {
  const text = `毒丹入腹，毒性真元升至 ${event.poison_level_after.toFixed(0)}，消化负荷压到 ${event.digestion_after.toFixed(0)}。${poisonSideEffectText(event.side_effect_tag)}`;
  return {
    scope: "player",
    target: `poison_dose:${event.player_entity_id}|tick:${event.at_tick}`,
    text,
    style: "narration",
  };
}

export function renderPoisonOverdoseNarration(event: PoisonOverdoseEventV1): Narration {
  const severityText = {
    mild: "轻微反噬",
    moderate: "中度反噬",
    severe: "重度反噬",
  }[event.severity];
  const tearText =
    event.micro_tear_probability > 0
      ? "经脉边缘传来细响，像旧瓷又添了一道微裂。"
      : "经脉暂未裂开，只是余毒还在腹中打转。";
  return {
    scope: "player",
    target: `poison_overdose:${event.player_entity_id}|tick:${event.at_tick}`,
    text: `${severityText}压住气息，寿元折去 ${event.lifespan_penalty_years.toFixed(1)} 年。${tearText}`,
    style: "narration",
  };
}

export interface PoisonTraitRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface PoisonTraitNarrationRuntimeConfig {
  sub: PoisonTraitRuntimeClient;
  pub: PoisonTraitRuntimeClient;
  logger?: Pick<Console, "info" | "warn">;
}

type PoisonRuntimePayload =
  | { kind: "dose"; payload: PoisonDoseEventV1 }
  | { kind: "overdose"; payload: PoisonOverdoseEventV1 };

export class PoisonTraitNarrationRuntime {
  private readonly sub: PoisonTraitRuntimeClient;
  private readonly pub: PoisonTraitRuntimeClient;
  private readonly logger: Pick<Console, "info" | "warn">;

  readonly stats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    void this.handlePayload(channel, message);
  };

  constructor(config: PoisonTraitNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    await this.sub.subscribe(POISON_DOSE_EVENT);
    await this.sub.subscribe(POISON_OVERDOSE_EVENT);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.logger.info(`[poison-trait-runtime] subscribed to ${POISON_DOSE_EVENT}, ${POISON_OVERDOSE_EVENT}`);
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
      this.logger.warn("[poison-trait-runtime] non-JSON payload:", error);
      return;
    }

    const event = parsePoisonRuntimePayload(channel, parsed);
    if (!event) {
      this.stats.rejectedContract += 1;
      return;
    }
    this.stats.received += 1;

    const narration = event.kind === "dose"
      ? renderPoisonDoseNarration(event.payload)
      : renderPoisonOverdoseNarration(event.payload);
    const envelope = { v: 1, narrations: [narration] };
    const validation = validateNarrationV1Contract(envelope);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[poison-trait-runtime] narration contract rejected");
      return;
    }

    await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
    this.stats.published += 1;
  }
}

function parsePoisonRuntimePayload(channel: string, parsed: unknown): PoisonRuntimePayload | null {
  if (channel === POISON_DOSE_EVENT) {
    return validatePoisonDoseEventV1(parsed).ok
      ? { kind: "dose", payload: parsed as PoisonDoseEventV1 }
      : null;
  }
  if (channel === POISON_OVERDOSE_EVENT) {
    return validatePoisonOverdoseEventV1(parsed).ok
      ? { kind: "overdose", payload: parsed as PoisonOverdoseEventV1 }
      : null;
  }
  return null;
}
