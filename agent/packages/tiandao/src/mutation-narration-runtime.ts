/**
 * Mutation narration runtime — consumes `bong:mutation_event` and produces
 * tiandao narration for stage 3+ (heavy / bestial) mutation advances.
 *
 * plan-dandao-path-v1 §6.4: 变异阶段 3+ 触发天道 narration。
 *
 * Pattern follows poison-trait-runtime.ts: subscribe → validate → render → publish.
 */

import type { MutationEventV1, MutationStageV1, Narration } from "@bong/schema";
import {
  CHANNELS,
  validateMutationEventV1,
  validateNarrationV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, MUTATION_EVENT } = CHANNELS;

// §6.4 narration templates — 天道对变异体的反应。
const STAGE_NARRATION_TEMPLATES: Record<string, string[]> = {
  heavy: [
    "天道感知到一股浊乱的气息——有什么东西不应该是那个形状。",
    "天穹微颤，某处灵脉的纹路像是被从内部撕开，又以另一种模样长合。",
    "你的经脉里多了几条不属于人的走向。天道将此记入劫册。",
  ],
  bestial: [
    "你感到天穹的目光——不是因为你境界高，而是因为你的身体已经不像人了。",
    "天道的注视从审量变为判定。你的形骸在天地间留下了兽类才有的痕迹。",
    "劫雷正在成形。不是因为你逆天，而是因为天已不认你是人。",
  ],
};

/**
 * Pick a narration template based on stage. Uses tick as hash seed for
 * deterministic but varied selection.
 */
function pickTemplate(stage: MutationStageV1, tick: number): string {
  const templates = STAGE_NARRATION_TEMPLATES[stage];
  if (!templates || templates.length === 0) {
    return "天道感知到一股浊乱的气息。";
  }
  return templates[tick % templates.length];
}

/** Returns true if the mutation stage triggers tiandao narration (3+). */
export function shouldNarrate(stage: MutationStageV1): boolean {
  return stage === "heavy" || stage === "bestial";
}

/**
 * Render a narration for a mutation advance event.
 * Only called for stage 3+ (heavy / bestial).
 */
export function renderMutationNarration(event: MutationEventV1): Narration {
  const text = pickTemplate(event.to_stage, event.at_tick);
  return {
    scope: event.to_stage === "bestial" ? "zone" : "player",
    target: `mutation:${event.entity_id}|stage:${event.to_stage}|tick:${event.at_tick}`,
    text,
    style: event.to_stage === "bestial" ? "perception" : "narration",
  };
}

export interface MutationNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface MutationNarrationRuntimeConfig {
  sub: MutationNarrationRuntimeClient;
  pub: MutationNarrationRuntimeClient;
  logger?: Pick<Console, "info" | "warn">;
}

export class MutationNarrationRuntime {
  private readonly sub: MutationNarrationRuntimeClient;
  private readonly pub: MutationNarrationRuntimeClient;
  private readonly logger: Pick<Console, "info" | "warn">;

  readonly stats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    skippedLowStage: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    void this.handlePayload(channel, message);
  };

  constructor(config: MutationNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    await this.sub.subscribe(MUTATION_EVENT);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.logger.info(`[mutation-narration-runtime] subscribed to ${MUTATION_EVENT}`);
  }

  async disconnect(): Promise<void> {
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(channel: string, message: string): Promise<void> {
    if (channel !== MUTATION_EVENT) return;

    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[mutation-narration-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateMutationEventV1(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[mutation-narration-runtime] contract validation failed");
      return;
    }

    const event = parsed as MutationEventV1;
    this.stats.received += 1;

    // §6.4: only stage 3+ (heavy / bestial) triggers narration
    if (!shouldNarrate(event.to_stage)) {
      this.stats.skippedLowStage += 1;
      return;
    }

    const narration = renderMutationNarration(event);
    const envelope = { v: 1, narrations: [narration] };
    const narrationValidation = validateNarrationV1Contract(envelope);
    if (!narrationValidation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[mutation-narration-runtime] narration contract rejected");
      return;
    }

    await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
    this.stats.published += 1;
  }
}
