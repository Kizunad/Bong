import { describe, expect, it } from "vitest";

import { CHANNELS } from "@bong/schema";

import { createMockClient } from "../src/llm.js";
import {
  renderWoliuV2Narration,
  WoliuV2NarrationRuntime,
  type WoliuV2RuntimeClient,
} from "../src/woliu_v2_runtime.js";

class FakeRedis implements WoliuV2RuntimeClient {
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

describe("WoliuV2NarrationRuntime", () => {
  it("subscribes to cast, backfire, and turbulence channels", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new WoliuV2NarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
      logger: console,
    });

    await runtime.connect();

    expect(sub.subscribed).toEqual([
      CHANNELS.WOLIU_V2_CAST,
      CHANNELS.WOLIU_V2_BACKFIRE,
      CHANNELS.WOLIU_V2_TURBULENCE,
    ]);
  });

  it("renders deterministic fallback for every woliu v2 skill", () => {
    for (const skill of ["hold", "burst", "mouth", "pull", "heart"] as const) {
      const narration = renderWoliuV2Narration({
        kind: "cast",
        payload: {
          caster: "player:kiz",
          skill,
          tick: 1,
          lethal_radius: 1,
          influence_radius: 1,
          turbulence_radius: 1,
          absorbed_qi: 0.01,
          swirl_qi: 1,
          animation_id: "bong:vortex_palm_open",
          particle_id: "bong:vortex_spiral",
          sound_recipe_id: "vortex_low_hum",
          icon_texture: "bong:textures/gui/skill/woliu_hold.png",
        },
      });
      expect(narration.text).toContain("九成九");
    }
  });

  it("publishes narration for valid turbulence payloads", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new WoliuV2NarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
      logger: console,
    });
    await runtime.connect();

    sub.emit(CHANNELS.WOLIU_V2_TURBULENCE, {
      caster: "player:kiz",
      skill: "heart",
      center: [1, 64, 1],
      radius: 75,
      intensity: 1,
      swirl_qi: 99,
      tick: 9,
    });
    await Promise.resolve();

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(CHANNELS.AGENT_NARRATE);
  });
});
