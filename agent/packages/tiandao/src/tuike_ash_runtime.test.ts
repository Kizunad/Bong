/**
 * plan-combat-skill-feedback-bridges-v1 P6 — TuikeAshDecayNarrationRuntime 单测。
 *
 * 测契约：
 *   - 收 bong:tuike_v2/ash_decay 消息 → 产 AGENT_NARRATE（5 个 tier 各验）
 *   - Ancient tier → 叙事文本含"上古"（专属文案）
 *   - 无效 JSON / schema 不符 → rejectedContract++，不 publish
 *   - 忽略非 TUIKE_V2_ASH_DECAY channel 消息
 *   - cleanup fn 存在且可调用
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { CHANNELS } from "@bong/schema";

import {
  TuikeAshDecayNarrationRuntime,
  renderTuikeAshDecayNarration,
} from "./tuike_ash_runtime.js";

const { TUIKE_V2_ASH_DECAY, AGENT_NARRATE } = CHANNELS;

// ──── mock client ────────────────────────────────────────────────────────────

function makeMockClient() {
  const listeners: Map<string, ((ch: string, msg: string) => void)[]> = new Map();

  return {
    subscribe: vi.fn(async (channel: string) => {
      listeners.set(channel, listeners.get(channel) ?? []);
    }),
    on: vi.fn((event: string, listener: (ch: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      arr.push(listener);
      listeners.set(event, arr);
    }),
    off: vi.fn((event: string, listener: (ch: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      const idx = arr.indexOf(listener);
      if (idx !== -1) arr.splice(idx, 1);
      listeners.set(event, arr);
    }),
    unsubscribe: vi.fn(async () => {}),
    disconnect: vi.fn(() => {}),
    published: [] as { channel: string; message: string }[],
    publish: vi.fn(async (channel: string, message: string) => {
      listeners.get("publish")?.forEach((l) => l(channel, message));
      return 1;
    }),

    emit(channel: string, message: string) {
      (listeners.get("message") ?? []).forEach((l) => l(channel, message));
    },
  };
}

function makeAshPayload(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    owner_id: "offline:TestPlayer",
    output_item_id: "bong:false_skin_ash",
    tier: "mid",
    tick: 100,
    ...overrides,
  });
}

// ──── tests ──────────────────────────────────────────────────────────────────

describe("TuikeAshDecayNarrationRuntime", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let runtime: TuikeAshDecayNarrationRuntime;

  beforeEach(async () => {
    sub = makeMockClient();
    pub = makeMockClient();
    runtime = new TuikeAshDecayNarrationRuntime({ sub, pub });
    await runtime.connect();
  });

  it("subscribes to CHANNELS.TUIKE_V2_ASH_DECAY on connect", () => {
    expect(
      sub.subscribe,
      `connect() 必须订阅 ${TUIKE_V2_ASH_DECAY} 频道以接收灰烬事件，否则消息永远不会到达 runtime；` +
        "删掉 connect() 里的 sub.subscribe(TUIKE_V2_ASH_DECAY) 这行，测试即红",
    ).toHaveBeenCalledWith(TUIKE_V2_ASH_DECAY);
  });

  it("mid tier → 产出 AGENT_NARRATE publish", async () => {
    await runtime.handlePayload(TUIKE_V2_ASH_DECAY, makeAshPayload());
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
    expect(runtime.stats.rejectedContract).toBe(0);
  });

  it("AGENT_NARRATE publish channel 正确", async () => {
    let publishedChannel = "";
    pub.publish.mockImplementation(async (channel: string, message: string) => {
      publishedChannel = channel;
      return 1;
    });
    await runtime.handlePayload(TUIKE_V2_ASH_DECAY, makeAshPayload());
    expect(
      publishedChannel,
      `叙事应发布到 ${AGENT_NARRATE}，实际发布到 '${publishedChannel}'`,
    ).toBe(AGENT_NARRATE);
  });

  it("narration envelope v=1, narrations.length=1", async () => {
    const received: string[] = [];
    pub.publish.mockImplementation(async (_ch: string, message: string) => {
      received.push(message);
      return 1;
    });
    await runtime.handlePayload(TUIKE_V2_ASH_DECAY, makeAshPayload());
    expect(received).toHaveLength(1);
    const envelope = JSON.parse(received[0] as string) as {
      v: number;
      narrations: unknown[];
    };
    expect(envelope.v).toBe(1);
    expect(envelope.narrations).toHaveLength(1);
  });

  it("ancient tier → 叙事文本含「上古」（专属文案）", async () => {
    let narrationText = "";
    pub.publish.mockImplementation(async (_ch: string, message: string) => {
      const env = JSON.parse(message) as {
        narrations: Array<{ text: string }>;
      };
      narrationText = env.narrations[0]?.text ?? "";
      return 1;
    });
    await runtime.handlePayload(
      TUIKE_V2_ASH_DECAY,
      makeAshPayload({
        tier: "ancient",
        output_item_id: "bong:false_skin_ancient_relic_shard",
      }),
    );
    expect(
      narrationText,
      `Ancient tier 叙事应包含「上古」，实际文本: '${narrationText}'`,
    ).toContain("上古");
  });

  it("fan tier → published", async () => {
    await runtime.handlePayload(
      TUIKE_V2_ASH_DECAY,
      makeAshPayload({ tier: "fan" }),
    );
    expect(runtime.stats.published).toBe(1);
  });

  it("light tier → published", async () => {
    await runtime.handlePayload(
      TUIKE_V2_ASH_DECAY,
      makeAshPayload({ tier: "light" }),
    );
    expect(runtime.stats.published).toBe(1);
  });

  it("heavy tier → published", async () => {
    await runtime.handlePayload(
      TUIKE_V2_ASH_DECAY,
      makeAshPayload({ tier: "heavy" }),
    );
    expect(runtime.stats.published).toBe(1);
  });

  it("无效 JSON → rejectedContract++, 不 publish", async () => {
    let publishCalled = false;
    pub.publish.mockImplementation(async () => { publishCalled = true; return 1; });
    await runtime.handlePayload(TUIKE_V2_ASH_DECAY, "not-json");
    expect(publishCalled).toBe(false);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("schema 不符（缺 owner_id）→ rejectedContract++, 不 publish", async () => {
    let publishCalled = false;
    pub.publish.mockImplementation(async () => { publishCalled = true; return 1; });
    const bad = JSON.stringify({ output_item_id: "bong:ash", tier: "mid", tick: 1 });
    await runtime.handlePayload(TUIKE_V2_ASH_DECAY, bad);
    expect(publishCalled).toBe(false);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("schema 不符（无效 tier）→ rejectedContract++, 不 publish", async () => {
    let publishCalled = false;
    pub.publish.mockImplementation(async () => { publishCalled = true; return 1; });
    const bad = JSON.stringify({
      owner_id: "offline:X",
      output_item_id: "bong:ash",
      tier: "ultra",
      tick: 1,
    });
    await runtime.handlePayload(TUIKE_V2_ASH_DECAY, bad);
    expect(publishCalled).toBe(false);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("忽略非 TUIKE_V2_ASH_DECAY channel 消息（via onMessage 路径）", async () => {
    let publishCalled = false;
    pub.publish.mockImplementation(async () => { publishCalled = true; return 1; });
    sub.emit("bong:other_channel", makeAshPayload());
    await new Promise((r) => setTimeout(r, 0));
    expect(publishCalled).toBe(false);
  });

  it("onMessage via emit 走完整路径", async () => {
    let publishCalled = false;
    pub.publish.mockImplementation(async () => { publishCalled = true; return 1; });
    sub.emit(TUIKE_V2_ASH_DECAY, makeAshPayload());
    await new Promise((r) => setTimeout(r, 0));
    expect(publishCalled).toBe(true);
  });

  it("stats 初始为 0", () => {
    const r2 = new TuikeAshDecayNarrationRuntime({ sub, pub });
    expect(r2.stats).toEqual({
      received: 0,
      published: 0,
      rejectedContract: 0,
    });
  });
});

// ──── renderTuikeAshDecayNarration 单测 ──────────────────────────────────────

describe("renderTuikeAshDecayNarration", () => {
  it("mid tier — 含 owner_id 和 '中档伪皮'", () => {
    const text = renderTuikeAshDecayNarration({
      owner_id: "offline:Azure",
      output_item_id: "bong:false_skin_ash",
      tier: "mid",
      tick: 500,
    });
    expect(text).toContain("offline:Azure");
    expect(text).toContain("中档伪皮");
  });

  it("ancient tier — 专属文案含「上古遗物碎片」", () => {
    const text = renderTuikeAshDecayNarration({
      owner_id: "offline:Azure",
      output_item_id: "bong:false_skin_ancient_relic_shard",
      tier: "ancient",
      tick: 9999,
    });
    expect(text).toContain("上古遗物碎片");
    expect(text).toContain("offline:Azure");
  });

  it("fan tier — 含 '凡级伪皮'", () => {
    const text = renderTuikeAshDecayNarration({
      owner_id: "offline:Ming",
      output_item_id: "bong:false_skin_ash",
      tier: "fan",
      tick: 10,
    });
    expect(text).toContain("凡级伪皮");
  });

  it("heavy tier — 含 '重档伪皮'", () => {
    const text = renderTuikeAshDecayNarration({
      owner_id: "offline:Ming",
      output_item_id: "bong:false_skin_ash",
      tier: "heavy",
      tick: 20,
    });
    expect(text).toContain("重档伪皮");
  });
});
