import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  DuguNarrationRuntime,
  type DuguNarrationRuntimeClient,
} from "../src/dugu-narration.js";
import type { LlmClient } from "../src/llm.js";

const { AGENT_NARRATE, DUGU_POISON_PROGRESS, DUGU_ANTIDOTE_RESULT } = CHANNELS;

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

const antidoteSuccessPayload = {
  healer: "player:bob",
  target: "player:alice",
  result: "success" as const,
  meridian_id: "Heart",
  qi_max_after: 120,
  tick: 6100,
};

const antidoteFailedPayload = {
  healer: "player:bob",
  target: "player:alice",
  result: "failed" as const,
  meridian_id: "LungLeft",
  qi_max_after: 95,
  tick: 6200,
};

describe("DuguNarrationRuntime", () => {
  it("subscribes to both poison_progress and antidote_result channels", async () => {
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

    expect(sub.subscribedChannels).toContain(DUGU_POISON_PROGRESS);
    expect(sub.subscribedChannels).toContain(DUGU_ANTIDOTE_RESULT);
    expect(sub.subscribedChannels).toHaveLength(2);
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

  it("publishes player-scoped LLM narration for antidote_result success", async () => {
    const pub = new FakePubSub();
    const runtime = new DuguNarrationRuntime({
      llm: makeLlm(
        JSON.stringify({ text: "心脉蛊毒尽散，真元重归清明。", style: "narration" }),
      ),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(DUGU_ANTIDOTE_RESULT, JSON.stringify(antidoteSuccessPayload));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "player",
      target: "player:alice",
      text: "心脉蛊毒尽散，真元重归清明。",
      style: "narration",
    });
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
  });

  it("publishes player-scoped LLM narration for antidote_result failure", async () => {
    const pub = new FakePubSub();
    const runtime = new DuguNarrationRuntime({
      llm: makeLlm(
        JSON.stringify({ text: "蛊毒根深，解毒之术无从奏效。", style: "narration" }),
      ),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(DUGU_ANTIDOTE_RESULT, JSON.stringify(antidoteFailedPayload));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "player",
      target: "player:alice",
      text: "蛊毒根深，解毒之术无从奏效。",
      style: "narration",
    });
  });

  it("falls back to antidote fallback narration when LLM fails (success result)", async () => {
    const pub = new FakePubSub();
    const runtime = new DuguNarrationRuntime({
      llm: {
        async chat() {
          throw new Error("llm-down");
        },
      },
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(DUGU_ANTIDOTE_RESULT, JSON.stringify(antidoteSuccessPayload));

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].target).toBe("player:alice");
    expect(String(envelope.narrations[0].text)).toContain("Heart");
    expect(String(envelope.narrations[0].text)).toContain("120");
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("falls back to antidote fallback narration when LLM fails (failed result)", async () => {
    const pub = new FakePubSub();
    const runtime = new DuguNarrationRuntime({
      llm: {
        async chat() {
          throw new Error("llm-down");
        },
      },
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(DUGU_ANTIDOTE_RESULT, JSON.stringify(antidoteFailedPayload));

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].target).toBe("player:alice");
    expect(String(envelope.narrations[0].text)).toContain("失败");
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("increments rejectedContract for invalid antidote_result payload", async () => {
    const pub = new FakePubSub();
    const runtime = new DuguNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(DUGU_ANTIDOTE_RESULT, JSON.stringify({ bad: "data" }));

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
    expect(runtime.stats.received).toBe(0);
  });

  it("ignores antidote_result messages on wrong channel (onMessage guard)", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new DuguNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.connect();
    // Fire a message on an unrelated channel
    sub.listeners.forEach((fn) =>
      fn("bong:other/channel", JSON.stringify(antidoteSuccessPayload)),
    );

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.received).toBe(0);
  });
});
