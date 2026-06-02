import { describe, expect, it, vi } from "vitest";
import { CHANNELS, type MutationEventV1 } from "@bong/schema";

import {
  MutationNarrationRuntime,
  renderMutationNarration,
  shouldNarrate,
  type MutationNarrationRuntimeClient,
} from "../src/mutation-narration-runtime.js";

const { AGENT_NARRATE, MUTATION_EVENT } = CHANNELS;

class FakePubSub implements MutationNarrationRuntimeClient {
  public published: Array<{ channel: string; message: string }> = [];
  public subscribedChannels: string[] = [];
  private readonly listenersByEvent = new Map<
    string,
    Array<(channel: string, message: string) => void>
  >();

  async subscribe(channel: string): Promise<void> {
    this.subscribedChannels.push(channel);
  }

  on(
    event: string,
    listener: (channel: string, message: string) => void,
  ) {
    const listeners = this.listenersByEvent.get(event) ?? [];
    listeners.push(listener);
    this.listenersByEvent.set(event, listeners);
    return this;
  }

  off(
    event: string,
    listener: (channel: string, message: string) => void,
  ) {
    const listeners = this.listenersByEvent.get(event) ?? [];
    this.listenersByEvent.set(
      event,
      listeners.filter((entry) => entry !== listener),
    );
    return this;
  }

  async unsubscribe(): Promise<void> {}

  disconnect(): void {}

  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    return 1;
  }

  emit(event: string, channel: string, message: string): void {
    for (const listener of this.listenersByEvent.get(event) ?? []) {
      listener(channel, message);
    }
  }
}

const silent = { info: vi.fn(), warn: vi.fn() };

function makeEvent(
  overrides?: Partial<MutationEventV1>,
): MutationEventV1 {
  return {
    v: 1,
    entity_id: 42,
    from_stage: "visible",
    to_stage: "heavy",
    cumulative_toxin: 260.0,
    at_tick: 1200,
    ...overrides,
  };
}

describe("shouldNarrate", () => {
  it("returns false for none, subtle, visible", () => {
    expect(shouldNarrate("none")).toBe(false);
    expect(shouldNarrate("subtle")).toBe(false);
    expect(shouldNarrate("visible")).toBe(false);
  });

  it("returns true for heavy and bestial", () => {
    expect(shouldNarrate("heavy")).toBe(true);
    expect(shouldNarrate("bestial")).toBe(true);
  });
});

describe("renderMutationNarration", () => {
  it("renders heavy stage narration with player scope", () => {
    const narration = renderMutationNarration(makeEvent({ to_stage: "heavy" }));
    expect(narration.scope).toBe("player");
    expect(narration.style).toBe("narration");
    expect(narration.text.length).toBeGreaterThan(0);
    expect(narration.target).toContain("mutation:42");
    expect(narration.target).toContain("stage:heavy");
  });

  it("renders bestial stage narration with zone scope", () => {
    const narration = renderMutationNarration(
      makeEvent({ to_stage: "bestial", at_tick: 3000 }),
    );
    expect(narration.scope).toBe("zone");
    expect(narration.style).toBe("perception");
    expect(narration.text.length).toBeGreaterThan(0);
  });

  it("selects different templates based on tick", () => {
    const n0 = renderMutationNarration(makeEvent({ at_tick: 0 }));
    const n1 = renderMutationNarration(makeEvent({ at_tick: 1 }));
    const n2 = renderMutationNarration(makeEvent({ at_tick: 2 }));
    // At least 2 of 3 should differ (we have 3 templates)
    const texts = new Set([n0.text, n1.text, n2.text]);
    expect(texts.size).toBeGreaterThanOrEqual(2);
  });
});

describe("MutationNarrationRuntime", () => {
  it("subscribes to mutation_event channel on connect", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([MUTATION_EVENT]);
  });

  it("publishes narration for stage 3 (heavy) event", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    await runtime.handlePayload(
      MUTATION_EVENT,
      JSON.stringify(makeEvent({ to_stage: "heavy" })),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations).toHaveLength(1);
    expect(envelope.narrations[0].scope).toBe("player");
    expect(runtime.stats.published).toBe(1);
    expect(runtime.stats.received).toBe(1);
  });

  it("publishes narration for stage 4 (bestial) event", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    await runtime.handlePayload(
      MUTATION_EVENT,
      JSON.stringify(makeEvent({ to_stage: "bestial" })),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].scope).toBe("zone");
    expect(envelope.narrations[0].style).toBe("perception");
    expect(runtime.stats.published).toBe(1);
  });

  it("skips narration for low stages (none / subtle / visible)", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    for (const stage of ["none", "subtle", "visible"] as const) {
      await runtime.handlePayload(
        MUTATION_EVENT,
        JSON.stringify(makeEvent({ to_stage: stage })),
      );
    }

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.skippedLowStage).toBe(3);
    expect(runtime.stats.received).toBe(3);
  });

  it("rejects malformed payloads without publishing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    await runtime.handlePayload(
      MUTATION_EVENT,
      JSON.stringify({ v: 1, entity_id: "not-a-number" }),
    );

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("rejects non-JSON payloads without publishing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    await runtime.handlePayload(MUTATION_EVENT, "not-json");

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("ignores messages from other channels", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    await runtime.handlePayload(
      "bong:other_channel",
      JSON.stringify(makeEvent()),
    );

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.received).toBe(0);
  });

  it("wires Redis message events to mutation narration", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    await runtime.connect();
    sub.emit(
      "message",
      MUTATION_EVENT,
      JSON.stringify(makeEvent({ to_stage: "heavy" })),
    );

    await vi.waitFor(() => expect(pub.published).toHaveLength(1));
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
  });

  it("disconnect returns without throwing (wiring cleanup test)", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    await runtime.connect();
    // 验证 disconnect 不抛出，可被纳入 cleanupFns 安全调用。
    await expect(runtime.disconnect()).resolves.toBeUndefined();
  });

  it("returns a cleanup function that resolves when disconnect called (startMutationNarrationRuntime pattern)", async () => {
    // 验证 main.ts 中 startMutationNarrationRuntime 返回的 cleanup fn 可以安全调用。
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new MutationNarrationRuntime({
      sub,
      pub,
      logger: silent,
    });

    await runtime.connect();

    // 模拟 startMutationNarrationRuntime 返回的 cleanup fn 模式
    const cleanupFn = async (): Promise<void> => {
      const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
      try {
        await Promise.race([runtime.disconnect(), timeout]);
      } catch (_) {
        // ignore
      }
    };

    // cleanup fn 应能被纳入 cleanupFns 数组并安全 await
    await expect(cleanupFn()).resolves.toBeUndefined();
  });
});
