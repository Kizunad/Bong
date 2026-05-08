import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  VoidActionNarrationRuntime,
  type VoidActionNarrationRuntimeClient,
} from "../src/void-actions-runtime.js";
import type { LlmClient } from "../src/llm.js";

const {
  AGENT_NARRATE,
  VOID_ACTION_BARRIER,
  VOID_ACTION_EXPLODE_ZONE,
  VOID_ACTION_LEGACY_ASSIGN,
  VOID_ACTION_SUPPRESS_TSY,
} = CHANNELS;

class FakePubSub implements VoidActionNarrationRuntimeClient {
  public published: Array<{ channel: string; message: string }> = [];
  public subscribedChannels: string[] = [];
  public listeners: Array<(channel: string, message: string) => void> = [];

  async subscribe(channel: string): Promise<void> {
    this.subscribedChannels.push(channel);
  }

  on(event: string, listener: (channel: string, message: string) => void) {
    if (event === "message") {
      this.listeners.push(listener);
    }
    return this;
  }

  off(event: string, listener: (channel: string, message: string) => void) {
    if (event === "message") {
      this.listeners = this.listeners.filter((entry) => entry !== listener);
    }
    return this;
  }

  async unsubscribe(): Promise<void> {}

  disconnect(): void {}

  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    return 1;
  }

  emit(channel: string, message: string): void {
    for (const listener of this.listeners) {
      listener(channel, message);
    }
  }
}

const failingLlm: LlmClient = {
  async chat() {
    throw new Error("boom");
  },
};

function payload(kind = "barrier") {
  return {
    v: 1,
    kind,
    actor_id: "offline:Void",
    actor_name: "Void",
    target: "spawn",
    at_tick: 42,
    qi_cost: 150,
    lifespan_cost_years: 30,
    scope: "broadcast",
    public_text: "Void 在 spawn 立下化虚障，道伥过线折其半气。",
  };
}

describe("VoidActionNarrationRuntime", () => {
  it("subscribes to all void action channels", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new VoidActionNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: { info: vi.fn(), warn: vi.fn() },
      systemPrompt: "test",
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([
      VOID_ACTION_SUPPRESS_TSY,
      VOID_ACTION_EXPLODE_ZONE,
      VOID_ACTION_BARRIER,
      VOID_ACTION_LEGACY_ASSIGN,
    ]);
  });

  it("falls back to public broadcast text when LLM fails", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new VoidActionNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: { info: vi.fn(), warn: vi.fn() },
      systemPrompt: "test",
    });

    await runtime.handlePayload(VOID_ACTION_BARRIER, JSON.stringify(payload()));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "void_action:barrier|actor:offline:Void|target:spawn|tick:42",
      text: "Void 在 spawn 立下化虚障，道伥过线折其半气。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("rejects channel and kind mismatch", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new VoidActionNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: { info: vi.fn(), warn: vi.fn() },
      systemPrompt: "test",
    });

    await runtime.handlePayload(VOID_ACTION_EXPLODE_ZONE, JSON.stringify(payload("barrier")));

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("serializes pubsub payload handling", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    let inFlight = 0;
    let maxInFlight = 0;
    const llm: LlmClient = {
      async chat() {
        inFlight += 1;
        maxInFlight = Math.max(maxInFlight, inFlight);
        await new Promise((resolve) => setTimeout(resolve, 5));
        inFlight -= 1;
        return JSON.stringify({ text: "虚风已过。", style: "narration" });
      },
    };
    const runtime = new VoidActionNarrationRuntime({
      llm,
      model: "mock",
      sub,
      pub,
      logger: { info: vi.fn(), warn: vi.fn() },
      systemPrompt: "test",
    });

    await runtime.connect();
    sub.emit(VOID_ACTION_BARRIER, JSON.stringify(payload("barrier")));
    sub.emit(VOID_ACTION_BARRIER, JSON.stringify({ ...payload("barrier"), at_tick: 43 }));
    await new Promise((resolve) => setTimeout(resolve, 30));

    expect(pub.published).toHaveLength(2);
    expect(maxInFlight).toBe(1);
  });
});
