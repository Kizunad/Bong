import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import { WoliuNarrationRuntime, type WoliuNarrationRuntimeClient } from "../src/woliu-narration.js";
import type { LlmClient } from "../src/llm.js";

const { AGENT_NARRATE, WOLIU_BACKFIRE, WOLIU_PROJECTILE_DRAINED } = CHANNELS;

class FakePubSub implements WoliuNarrationRuntimeClient {
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

describe("WoliuNarrationRuntime", () => {
  it("subscribes to both woliu event channels", async () => {
    const sub = new FakePubSub();
    const runtime = new WoliuNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub: new FakePubSub(),
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([WOLIU_BACKFIRE, WOLIU_PROJECTILE_DRAINED]);
  });

  it("publishes LLM narration for a backfire event", async () => {
    const pub = new FakePubSub();
    const runtime = new WoliuNarrationRuntime({
      llm: makeLlm(JSON.stringify({ text: "涡流倒卷，掌中一脉冷了下去。", style: "narration" })),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      WOLIU_BACKFIRE,
      JSON.stringify({
        caster: "entity:1",
        cause: "exceed_maintain_max",
        meridian_severed: "Lung",
        tick: 100,
        env_qi: 0.9,
        delta: 0.25,
        resisted: false,
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "woliu:backfire|caster:entity:1|tick:100",
      text: "涡流倒卷，掌中一脉冷了下去。",
      style: "narration",
    });
  });

  it("falls back for projectile drain when LLM fails", async () => {
    const pub = new FakePubSub();
    const runtime = new WoliuNarrationRuntime({
      llm: {
        async chat() {
          throw new Error("boom");
        },
      },
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      WOLIU_PROJECTILE_DRAINED,
      JSON.stringify({
        field_caster: "entity:caster",
        projectile: "entity:needle",
        drained_amount: 0.8,
        remaining_payload: 0.1,
        delta: 0.65,
        tick: 120,
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].target).toBe(
      "woliu:drain|caster:entity:caster|projectile:entity:needle|tick:120",
    );
    expect(String(envelope.narrations[0].text)).toContain("涡流");
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });
});
