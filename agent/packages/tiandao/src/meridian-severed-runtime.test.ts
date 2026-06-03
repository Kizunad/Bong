/**
 * plan-combat-skill-feedback-bridges-v1 P0 — MeridianSeveredNarrationRuntime 单测。
 *
 * 测契约：
 *   - 收 bong:meridian_severed 消息 → 产 AGENT_NARRATE（7 类来源各验）
 *   - 无效 JSON / schema 不符 → rejectedContract++，不 publish
 *   - VoluntarySever 发 narration（叙事仍发，VFX 由 server 层过滤）
 *   - cleanup fn 存在且可调用（main.ts startAuxiliaryRuntimes 返回数组）
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { CHANNELS } from "@bong/schema";

import { MeridianSeveredNarrationRuntime } from "./meridian-severed-runtime.js";

const { MERIDIAN_SEVERED, AGENT_NARRATE } = CHANNELS;

// ──── mock client ────────────────────────────────────────────────────────────

function makeMockClient() {
  const listeners: Map<string, ((ch: string, msg: string) => void)[]> =
    new Map();

  return {
    subscribed: [] as string[],
    published: [] as { channel: string; message: string }[],
    disconnected: false,

    subscribe: vi.fn(async (channel: string) => {
      listeners.set(channel, listeners.get(channel) ?? []);
    }),
    on: vi.fn((event: string, listener: (ch: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      arr.push(listener);
      listeners.set(event, arr);
    }),
    off: vi.fn(
      (event: string, listener: (ch: string, msg: string) => void) => {
        const arr = listeners.get(event) ?? [];
        const idx = arr.indexOf(listener);
        if (idx !== -1) arr.splice(idx, 1);
        listeners.set(event, arr);
      },
    ),
    unsubscribe: vi.fn(async () => {}),
    disconnect: vi.fn(() => {
      /* no-op */
    }),
    publish: vi.fn(async (channel: string, message: string) => {
      listeners.get("publish")?.forEach((l) => l(channel, message));
      return 1;
    }),

    // 触发 message 事件（模拟 Redis subscribe message）
    emit(channel: string, message: string) {
      (listeners.get("message") ?? []).forEach((l) => l(channel, message));
    },
  };
}

function makeSeveredPayload(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    v: 1,
    type: "meridian_severed",
    entity_id: "offline:TestPlayer",
    meridian_id: "Lung",
    source: "CombatWound",
    at_tick: 100,
    ...overrides,
  });
}

// ──── tests ──────────────────────────────────────────────────────────────────

describe("MeridianSeveredNarrationRuntime", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let runtime: MeridianSeveredNarrationRuntime;

  beforeEach(async () => {
    sub = makeMockClient();
    pub = makeMockClient();
    runtime = new MeridianSeveredNarrationRuntime({ sub, pub });
    await runtime.connect();
  });

  it("subscribes to CHANNELS.MERIDIAN_SEVERED on connect", () => {
    expect(
      sub.subscribe,
      `connect() 应订阅 ${MERIDIAN_SEVERED} 频道以接收断脉事件，否则消息永远不会到达 runtime`,
    ).toHaveBeenCalledWith(MERIDIAN_SEVERED);
  });

  it("CombatWound → 产出 AGENT_NARRATE publish", async () => {
    await runtime.handlePayload(MERIDIAN_SEVERED, makeSeveredPayload());
    expect(
      pub.publish,
      "CombatWound 断脉事件应触发恰好一次 pub.publish，确保叙事不会重复发送",
    ).toHaveBeenCalledOnce();
    const [channel, msg] = pub.publish.mock.calls[0] as [string, string];
    expect(
      channel,
      `叙事应发布到 ${AGENT_NARRATE} 频道，而非其他频道`,
    ).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(msg) as { v: number; narrations: unknown[] };
    expect(envelope.v, "payload 版本必须是 v:1 以匹配 schema 约定").toBe(1);
    expect(
      envelope.narrations,
      "每次断脉事件应产出恰好 1 条叙事，多了会淹没玩家，少了会漏叙事",
    ).toHaveLength(1);
    expect(
      runtime.stats.received,
      "处理 1 条消息后 stats.received 应为 1",
    ).toBe(1);
    expect(
      runtime.stats.published,
      "成功 publish 叙事后 stats.published 应为 1",
    ).toBe(1);
  });

  it("VoluntarySever → 也产出 AGENT_NARRATE（叙事层不过滤，VFX 由 server 层过滤）", async () => {
    await runtime.handlePayload(
      MERIDIAN_SEVERED,
      makeSeveredPayload({ source: "VoluntarySever", meridian_id: "Du" }),
    );
    expect(pub.publish).toHaveBeenCalledOnce();
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
  });

  it("BackfireOverload → 产出 narration text 含经脉中文名", async () => {
    await runtime.handlePayload(
      MERIDIAN_SEVERED,
      makeSeveredPayload({
        source: "BackfireOverload",
        meridian_id: "Heart",
      }),
    );
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as {
      narrations: Array<{ text: string }>;
    };
    const text = envelope.narrations[0]?.text ?? "";
    expect(text).toContain("手少阴心经");
  });

  it("OverloadTear → published", async () => {
    await runtime.handlePayload(
      MERIDIAN_SEVERED,
      makeSeveredPayload({ source: "OverloadTear" }),
    );
    expect(runtime.stats.published).toBe(1);
  });

  it("TribulationFail → published", async () => {
    await runtime.handlePayload(
      MERIDIAN_SEVERED,
      makeSeveredPayload({ source: "TribulationFail" }),
    );
    expect(runtime.stats.published).toBe(1);
  });

  it("DuguDistortion → published", async () => {
    await runtime.handlePayload(
      MERIDIAN_SEVERED,
      makeSeveredPayload({ source: "DuguDistortion" }),
    );
    expect(runtime.stats.published).toBe(1);
  });

  it("Other source → published（扩展来源走 narration）", async () => {
    await runtime.handlePayload(
      MERIDIAN_SEVERED,
      makeSeveredPayload({ source: { Other: "上古遗物反噬" } }),
    );
    expect(runtime.stats.published).toBe(1);
  });

  it("无效 JSON → rejectedContract++, 不 publish", async () => {
    await runtime.handlePayload(MERIDIAN_SEVERED, "not-json");
    expect(
      pub.publish,
      "无效 JSON 应被拒绝，不应触发任何 publish，防止乱码叙事下发给玩家",
    ).not.toHaveBeenCalled();
    expect(
      runtime.stats.rejectedContract,
      "无效 JSON 应令 rejectedContract 自增为 1，以便监控 schema 违规",
    ).toBe(1);
  });

  it("schema 不符（缺 meridian_id）→ rejectedContract++, 不 publish", async () => {
    const bad = JSON.stringify({ v: 1, type: "meridian_severed" });
    await runtime.handlePayload(MERIDIAN_SEVERED, bad);
    expect(
      pub.publish,
      "缺 meridian_id 的 payload 应被 schema 校验拒绝，不应触发 publish",
    ).not.toHaveBeenCalled();
    expect(
      runtime.stats.rejectedContract,
      "schema 不符应令 rejectedContract 自增为 1",
    ).toBe(1);
  });

  it("忽略非 MERIDIAN_SEVERED channel 消息（通过 onMessage 路径）", async () => {
    // onMessage listener filters by channel; emit on a different channel should be ignored
    sub.emit("bong:other_channel", makeSeveredPayload());
    await new Promise((r) => setTimeout(r, 0));
    expect(pub.publish).not.toHaveBeenCalled();
  });

  it("onMessage via emit 走完整路径", async () => {
    sub.emit(MERIDIAN_SEVERED, makeSeveredPayload());
    // handlePayload is async, wait microtasks
    await new Promise((r) => setTimeout(r, 0));
    expect(pub.publish).toHaveBeenCalledOnce();
  });

  it("disconnect 后不再处理消息", async () => {
    await runtime.disconnect();
    // 验证 cleanup 完成（合理的实现约束）
    expect(
      sub.off,
      "disconnect() 应调用 sub.off 移除 onMessage 监听器，否则消息仍会被处理",
    ).toHaveBeenCalled();
    expect(
      sub.unsubscribe,
      "disconnect() 应调用 sub.unsubscribe 退订频道，释放 Redis 订阅资源",
    ).toHaveBeenCalled();
    // 核心行为断言：disconnect 后触发消息，叙事不再被发布
    sub.emit(MERIDIAN_SEVERED, makeSeveredPayload());
    await new Promise((r) => setTimeout(r, 0));
    expect(
      pub.publish,
      "disconnect() 后收到的断脉消息不应触发任何 publish，runtime 必须彻底停止处理",
    ).not.toHaveBeenCalled();
  });

  it("stats 初始为 0", () => {
    const r2 = new MeridianSeveredNarrationRuntime({ sub, pub });
    expect(r2.stats).toEqual({
      received: 0,
      ignored: 0,
      published: 0,
      rejectedContract: 0,
    });
  });

  it("narration style 是 'narration'（非 perception）", async () => {
    await runtime.handlePayload(MERIDIAN_SEVERED, makeSeveredPayload());
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as {
      narrations: Array<{ style: string }>;
    };
    expect(envelope.narrations[0]?.style).toBe("narration");
  });

  it("narration scope 是 'player'", async () => {
    await runtime.handlePayload(MERIDIAN_SEVERED, makeSeveredPayload());
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as {
      narrations: Array<{ scope: string }>;
    };
    expect(envelope.narrations[0]?.scope).toBe("player");
  });
});
