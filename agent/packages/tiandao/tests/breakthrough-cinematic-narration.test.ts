import { describe, expect, it, vi } from "vitest";
import { CHANNELS, type BreakthroughCinematicEventV1 } from "@bong/schema";

import {
  BreakthroughCinematicNarrationRuntime,
  type BreakthroughCinematicNarrationRuntimeClient,
  fallbackBreakthroughCinematicNarration,
} from "../src/breakthrough-cinematic-narration.js";
import type { LlmClient } from "../src/llm.js";

const { AGENT_NARRATE, BREAKTHROUGH_CINEMATIC } = CHANNELS;

class FakePubSub implements BreakthroughCinematicNarrationRuntimeClient {
  public published: Array<{ channel: string; message: string }> = [];
  public subscribedChannels: string[] = [];
  public listeners: Array<(channel: string, message: string) => void> = [];

  async subscribe(channel: string): Promise<void> {
    this.subscribedChannels.push(channel);
  }

  on(_event: string, listener: (channel: string, message: string) => void) {
    this.listeners.push(listener);
    return this;
  }

  off(_event: string, listener: (channel: string, message: string) => void) {
    this.listeners = this.listeners.filter((entry) => entry !== listener);
    return this;
  }

  async unsubscribe(): Promise<void> {}

  disconnect(): void {}

  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    return 1;
  }
}

function makeLlm(content: string): LlmClient {
  return {
    async chat(model: string) {
      return { content, durationMs: 0, requestId: null, model };
    },
  };
}

const silent = { info: vi.fn(), warn: vi.fn() };

const apexPayload: BreakthroughCinematicEventV1 = {
  v: 1,
  actor_id: "char:alice",
  phase: "apex",
  phase_tick: 0,
  phase_duration_ticks: 120,
  realm_from: "Solidify",
  realm_to: "Spirit",
  result: "success",
  interrupted: false,
  world_pos: [128.5, 64, -32.25],
  visible_radius_blocks: 5000,
  global: true,
  distant_billboard: true,
  season_overlay: "adaptive",
  style: "sky_resonance",
  at_tick: 2400,
};

describe("BreakthroughCinematicNarrationRuntime", () => {
  it("subscribes to breakthrough cinematic channel", async () => {
    const sub = new FakePubSub();
    const runtime = new BreakthroughCinematicNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub: new FakePubSub(),
      logger: silent,
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([BREAKTHROUGH_CINEMATIC]);
  });

  it("publishes LLM narration with deterministic target", async () => {
    const pub = new FakePubSub();
    const runtime = new BreakthroughCinematicNarrationRuntime({
      llm: makeLlm(JSON.stringify({ text: "天光压下一瞬，某人入了通灵。", style: "narration" })),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
    });

    await runtime.handlePayload(JSON.stringify(apexPayload));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "breakthrough:char:alice|apex|Solidify->Spirit",
      text: "天光压下一瞬，某人入了通灵。",
      style: "narration",
    });
  });

  it("falls back when LLM is unavailable", async () => {
    const pub = new FakePubSub();
    const runtime = new BreakthroughCinematicNarrationRuntime({
      llm: {
        async chat() {
          throw new Error("offline");
        },
      },
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
    });

    await runtime.handlePayload(JSON.stringify(apexPayload));

    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].text).toBe(fallbackBreakthroughCinematicNarration(apexPayload).text);
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });
});
