import { describe, expect, it } from "vitest";

import { CHANNELS, validateTuikeV2SkillEventV1Contract } from "@bong/schema";

import { createMockClient } from "../src/llm.js";
import {
  renderTuikeV2Narration,
  TuikeV2NarrationRuntime,
  type TuikeV2RuntimeClient,
} from "../src/tuike_v2_runtime.js";

class FakeRedis implements TuikeV2RuntimeClient {
  readonly subscribed: string[] = [];
  readonly published: Array<{ channel: string; message: string }> = [];
  private listener: ((channel: string, message: string) => void) | null = null;

  async subscribe(channel: string): Promise<void> {
    this.subscribed.push(channel);
  }
  on(_event: string, listener: (channel: string, message: string) => void): void {
    this.listener = listener;
  }
  off(): void {
    this.listener = null;
  }
  async unsubscribe(): Promise<void> {}
  disconnect(): void {}
  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    return 1;
  }
  emit(channel: string, payload: unknown): void {
    this.listener?.(channel, JSON.stringify(payload));
  }
}

const validTransferPayload = {
  v: 1,
  type: "tuike_v2_skill_event",
  caster_id: "offline:Azure",
  skill_id: "transfer_taint",
  tier: "ancient",
  layers_after: 2,
  contam_moved_percent: 15,
  permanent_absorbed: 0.4,
  qi_cost: 105,
  contam_load: 15,
  tick: 9,
  animation_id: "bong:tuike_taint_transfer",
  particle_id: "bong:ancient_skin_glow",
  sound_recipe_id: "contam_transfer_hum",
  icon_texture: "bong-client:textures/gui/skill/tuike_transfer_taint.png",
} as const;

describe("TuikeV2NarrationRuntime", () => {
  it("schema accepts rust-shaped transfer payload", () => {
    expect(validateTuikeV2SkillEventV1Contract(validTransferPayload).ok).toBe(true);
  });

  it("subscribes to tuike v2 skill event channel", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new TuikeV2NarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
      logger: console,
    });

    await runtime.connect();

    expect(sub.subscribed).toEqual([CHANNELS.TUIKE_V2_SKILL_EVENT]);
  });

  it("renders permanent taint hard-counter narration", () => {
    const narration = renderTuikeV2Narration(validTransferPayload);

    expect(narration.text).toContain("毒蛊永久标记");
    expect(narration.target).toContain("tuike_v2:transfer");
  });

  it("publishes narration for valid skill payloads", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new TuikeV2NarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
      logger: console,
    });
    await runtime.connect();

    sub.emit(CHANNELS.TUIKE_V2_SKILL_EVENT, validTransferPayload);
    await Promise.resolve();

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(CHANNELS.AGENT_NARRATE);
  });
});
