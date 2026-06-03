import { describe, expect, it } from "vitest";

import { CHANNELS, type VoidErosionEventV1 } from "@bong/schema";

import { createMockClient } from "../src/llm.js";
import {
  renderVoidErosionNarration,
  VoidErosionNarrationRuntime,
  type VoidErosionRuntimeClient,
} from "../src/void_erosion_runtime.js";

// ── FakeRedis ─────────────────────────────────────────────────────────────────

class FakeRedis implements VoidErosionRuntimeClient {
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

// ── sample payloads ───────────────────────────────────────────────────────────

function sampleAdvance(
  from: VoidErosionEventV1["from_stage"],
  to: VoidErosionEventV1["to_stage"],
): VoidErosionEventV1 {
  return {
    entity: "player:kiz",
    from_stage: from,
    to_stage: to,
    cumulative_erosion: 25.0,
    server_tick: 1200,
  };
}

// ── renderVoidErosionNarration unit tests ────────────────────────────────────

describe("renderVoidErosionNarration", () => {
  it("None → LowPressure 生成非空叙事", () => {
    const event = sampleAdvance("none", "low_pressure");
    const narration = renderVoidErosionNarration(event);
    expect(narration.text.length).toBeGreaterThan(0);
    expect(narration.style).toBe("narration");
    expect(narration.scope).toBe("player");
  });

  it("EchoBody → VoidEroded 叙事提及虚蚀态", () => {
    const event: VoidErosionEventV1 = {
      entity: "player:kiz",
      from_stage: "echo_body",
      to_stage: "void_eroded",
      cumulative_erosion: 400.0,
      server_tick: 9000,
    };
    const narration = renderVoidErosionNarration(event);
    expect(narration.text.length).toBeGreaterThan(0);
    expect(narration.target).toContain("void_eroded");
  });

  it("all 4 transitions produce distinct non-empty texts", () => {
    const transitions: Array<[VoidErosionEventV1["from_stage"], VoidErosionEventV1["to_stage"]]> =
      [
        ["none", "low_pressure"],
        ["low_pressure", "void_shadow"],
        ["void_shadow", "echo_body"],
        ["echo_body", "void_eroded"],
      ];
    const texts = new Set<string>();
    for (const [from, to] of transitions) {
      const narration = renderVoidErosionNarration(sampleAdvance(from, to));
      expect(narration.text.length).toBeGreaterThan(0);
      texts.add(narration.text);
    }
    expect(texts.size).toBe(4); // all distinct
  });
});

// ── VoidErosionNarrationRuntime integration tests ────────────────────────────

describe("VoidErosionNarrationRuntime", () => {
  it("subscribes to void erosion event channel", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new VoidErosionNarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
      logger: console,
    });

    await runtime.connect();

    expect(sub.subscribed).toContain(CHANNELS.VOID_EROSION_EVENT);
  });

  it("publishes narration for valid void erosion advance event (None→LowPressure)", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new VoidErosionNarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
      logger: console,
    });
    await runtime.connect();

    sub.emit(CHANNELS.VOID_EROSION_EVENT, sampleAdvance("none", "low_pressure"));
    // Wait for async chain: message handler → llm.chat → pub.publish
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(CHANNELS.AGENT_NARRATE);
    const narrationEnvelope = JSON.parse(pub.published[0].message) as {
      v: number;
      narrations: Array<{ text: string; style: string }>;
    };
    expect(narrationEnvelope.v).toBe(1);
    expect(narrationEnvelope.narrations[0].text.length).toBeGreaterThan(0);
  });

  it("publishes narration for EchoBody→VoidEroded transition", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new VoidErosionNarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
      logger: console,
    });
    await runtime.connect();

    const event: VoidErosionEventV1 = {
      entity: "player:test",
      from_stage: "echo_body",
      to_stage: "void_eroded",
      cumulative_erosion: 405.0,
      server_tick: 5000,
    };
    sub.emit(CHANNELS.VOID_EROSION_EVENT, event);
    // Wait for async chain: message handler → llm.chat → pub.publish
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(CHANNELS.AGENT_NARRATE);
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
  });

  it("rejects invalid payload and increments rejectedContract counter", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new VoidErosionNarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
      logger: { info: () => {}, warn: () => {} },
    });
    await runtime.connect();

    // Missing required fields
    sub.emit(CHANNELS.VOID_EROSION_EVENT, { entity: "x" });
    await Promise.resolve();

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("ignores messages on unrelated channels", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new VoidErosionNarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
    });
    await runtime.connect();

    sub.emit("bong:unrelated", sampleAdvance("none", "low_pressure"));
    await Promise.resolve();

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.received).toBe(0);
  });
});
