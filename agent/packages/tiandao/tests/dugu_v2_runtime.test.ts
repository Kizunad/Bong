import { describe, expect, it } from "vitest";

import { CHANNELS } from "@bong/schema";

import { createMockClient } from "../src/llm.js";
import {
  DuguV2NarrationRuntime,
  renderDuguV2Narration,
  type DuguV2RuntimeClient,
} from "../src/dugu_v2_runtime.js";

class FakeRedis implements DuguV2RuntimeClient {
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

describe("DuguV2NarrationRuntime", () => {
  it("subscribes to cast, self-cure, and reverse channels", async () => {
    const sub = new FakeRedis();
    const runtime = new DuguV2NarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub: new FakeRedis(),
      logger: console,
    });

    await runtime.connect();

    expect(sub.subscribed).toEqual([
      CHANNELS.DUGU_V2_CAST,
      CHANNELS.DUGU_V2_SELF_CURE,
      CHANNELS.DUGU_V2_REVERSE,
    ]);
  });

  it("renders deterministic fallback for all five skills", () => {
    for (const skill of ["eclipse", "self_cure", "penetrate", "shroud", "reverse"] as const) {
      const narration = renderDuguV2Narration({
        kind: "cast",
        payload: {
          caster: "player:kiz",
          skill,
          tick: 1,
          hp_loss: 0,
          qi_loss: 0,
          qi_max_loss: 0,
          permanent_decay_rate_per_min: 0,
          returned_zone_qi: 0,
          reveal_probability: 0.01,
          animation_id: "bong:dugu_needle_throw",
          particle_id: "bong:dugu_taint_pulse",
          sound_recipe_id: "dugu_needle_hiss",
          icon_texture: "bong:textures/gui/skill/dugu_eclipse.png",
        },
      });
      expect(narration.text).toContain("player:kiz");
    }
  });

  it("publishes narration for valid reverse payloads", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new DuguV2NarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
      logger: console,
    });
    await runtime.connect();

    sub.emit(CHANNELS.DUGU_V2_REVERSE, {
      caster: "player:kiz",
      affected_targets: 3,
      burst_damage: 180,
      returned_zone_qi: 14.85,
      juebi_delay_ticks: 600,
      center: [1, 64, 1],
      tick: 9,
    });
    await Promise.resolve();

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(CHANNELS.AGENT_NARRATE);
  });
});
