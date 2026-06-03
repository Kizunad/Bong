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
  /** 发送原始字符串（用于测试非 JSON payload 拒绝路径）。 */
  emitRaw(channel: string, raw: string): void {
    this.listener?.(channel, raw);
  }
}

// ── async 工具 ────────────────────────────────────────────────────────────────

/**
 * 轮询等待 pub.published.length 达到期望数量，超时后 reject。
 * 防止 3× Promise.resolve() 在异步链更深时 flaky。
 */
async function waitForPublish(
  pub: FakeRedis,
  count: number,
  timeoutMs = 500,
): Promise<void> {
  const start = Date.now();
  while (pub.published.length < count) {
    if (Date.now() - start > timeoutMs) {
      throw new Error(
        `waitForPublish timeout after ${timeoutMs}ms: expected ${count} published, got ${pub.published.length}`,
      );
    }
    await new Promise((r) => setTimeout(r, 5));
  }
}

// ── sample payloads ───────────────────────────────────────────────────────────

function sampleAdvance(
  from: VoidErosionEventV1["from_stage"],
  to: VoidErosionEventV1["to_stage"],
): VoidErosionEventV1 {
  return {
    entity: "offline:kiz",
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
      entity: "offline:kiz",
      from_stage: "echo_body",
      to_stage: "void_eroded",
      cumulative_erosion: 400.0,
      server_tick: 9000,
    };
    const narration = renderVoidErosionNarration(event);
    expect(narration.text.length).toBeGreaterThan(0);
    expect(narration.target).toContain("void_eroded");
  });

  it("target 格式：entity_id 在第一个 '|' 之前（server 路由契约）", () => {
    // server normalize_player_target 取 target.split('|')[0] 与玩家 username/char_id 比对。
    // entity_id 必须是第一段，否则叙事永远路由不到玩家。
    const event = sampleAdvance("none", "low_pressure");
    const narration = renderVoidErosionNarration(event);
    const firstSegment = narration.target?.split("|")[0] ?? "";
    expect(firstSegment).toBe(event.entity);
  });

  it("target 格式与 meridian-severed runtime 对齐（entityId 首段）", () => {
    // 参考：meridian-severed-narration.ts target = `meridian_severed:${entity_id}|...`
    // 对于 void_erosion 同样 entity_id 在 | 前（不带前缀）确保 server 路由能命中
    const entities = ["offline:kiz", "char:12345", "offline:alice"];
    for (const entity of entities) {
      const event: VoidErosionEventV1 = {
        entity,
        from_stage: "none",
        to_stage: "low_pressure",
        cumulative_erosion: 20.0,
        server_tick: 100,
      };
      const narration = renderVoidErosionNarration(event);
      const firstSegment = narration.target?.split("|")[0] ?? "";
      expect(firstSegment).toBe(entity);
    }
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
    // 轮询等待异步链完成（message handler → llm.chat → pub.publish）
    await waitForPublish(pub, 1);

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
      entity: "offline:test",
      from_stage: "echo_body",
      to_stage: "void_eroded",
      cumulative_erosion: 405.0,
      server_tick: 5000,
    };
    sub.emit(CHANNELS.VOID_EROSION_EVENT, event);
    // 轮询等待异步链完成（message handler → llm.chat → pub.publish）
    await waitForPublish(pub, 1);

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

    // Missing required fields — contract validation rejects
    sub.emit(CHANNELS.VOID_EROSION_EVENT, { entity: "x" });
    await Promise.resolve();

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("非法 JSON payload → rejectedContract 递增，不发布叙事", async () => {
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

    // 发送非 JSON 字符串（parse 直接失败）
    sub.emitRaw(CHANNELS.VOID_EROSION_EVENT, "not valid json {{{{");
    await Promise.resolve();

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
    expect(runtime.stats.received).toBe(0);
  });

  it("LLM 调用失败 → fallback 叙事发布 + fallbackUsed 递增", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    // 总是 reject 的 LLM client
    const failingLlm = {
      async chat(_model: string): Promise<never> {
        throw new Error("LLM service unavailable");
      },
    };
    const runtime = new VoidErosionNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: { info: () => {}, warn: () => {} },
    });
    await runtime.connect();

    sub.emit(CHANNELS.VOID_EROSION_EVENT, sampleAdvance("none", "low_pressure"));
    await waitForPublish(pub, 1);

    // fallback 叙事仍应发布（不因 LLM 失败而丢弃）
    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(CHANNELS.AGENT_NARRATE);
    expect(runtime.stats.fallbackUsed).toBe(1);
    expect(runtime.stats.published).toBe(1);
  });

  it("LowPressure → VoidShadow 阶段推进发布叙事", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new VoidErosionNarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
    });
    await runtime.connect();

    sub.emit(CHANNELS.VOID_EROSION_EVENT, sampleAdvance("low_pressure", "void_shadow"));
    await waitForPublish(pub, 1);

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(CHANNELS.AGENT_NARRATE);
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
    const envelope = JSON.parse(pub.published[0].message) as { narrations: Array<{ text: string }> };
    expect(envelope.narrations[0].text.length).toBeGreaterThan(0);
  });

  it("VoidShadow → EchoBody 阶段推进发布叙事", async () => {
    const sub = new FakeRedis();
    const pub = new FakeRedis();
    const runtime = new VoidErosionNarrationRuntime({
      llm: createMockClient(),
      model: "mock",
      sub,
      pub,
    });
    await runtime.connect();

    const event: VoidErosionEventV1 = {
      entity: "offline:kiz",
      from_stage: "void_shadow",
      to_stage: "echo_body",
      cumulative_erosion: 200.0,
      server_tick: 3000,
    };
    sub.emit(CHANNELS.VOID_EROSION_EVENT, event);
    await waitForPublish(pub, 1);

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(CHANNELS.AGENT_NARRATE);
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
    const envelope = JSON.parse(pub.published[0].message) as { narrations: Array<{ text: string }> };
    expect(envelope.narrations[0].text.length).toBeGreaterThan(0);
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
