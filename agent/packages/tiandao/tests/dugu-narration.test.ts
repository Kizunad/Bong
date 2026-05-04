import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  DuguNarrationRuntime,
  type DuguNarrationRuntimeClient,
} from "../src/dugu-narration.js";
import type { LlmClient } from "../src/llm.js";

const { AGENT_NARRATE, DUGU_POISON_PROGRESS } = CHANNELS;

class FakePubSub implements DuguNarrationRuntimeClient {
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

const progressPayload = {
  target: "player:alice",
  attacker: "entity:dugu",
  meridian_id: "Heart",
  flow_capacity_after: 98,
  qi_max_after: 108,
  actual_loss_this_tick: 2,
  tick: 6000,
};

describe("DuguNarrationRuntime", () => {
  it("subscribes to poison progress channel", async () => {
    const sub = new FakePubSub();
    const runtime = new DuguNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub: new FakePubSub(),
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([DUGU_POISON_PROGRESS]);
  });

  it("publishes player-scoped LLM narration for poison progress", async () => {
    const pub = new FakePubSub();
    const runtime = new DuguNarrationRuntime({
      llm: makeLlm(JSON.stringify({ text: "心脉里像有冷线慢慢绞紧，真元上限又矮了一截。", style: "narration" })),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(DUGU_POISON_PROGRESS, JSON.stringify(progressPayload));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "player",
      target: "player:alice",
      text: "心脉里像有冷线慢慢绞紧，真元上限又矮了一截。",
      style: "narration",
    });
  });

  it("falls back when LLM fails", async () => {
    const pub = new FakePubSub();
    const runtime = new DuguNarrationRuntime({
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

    await runtime.handlePayload(DUGU_POISON_PROGRESS, JSON.stringify(progressPayload));

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].target).toBe("player:alice");
    expect(String(envelope.narrations[0].text)).toContain("真元上限");
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });
});
