/**
 * plan-dying-elder-v1 P3 — ElderEncounterNarrationRuntime 单测。
 *
 * 测契约：
 * - 收 bong:elder_encounter 消息 → 产 AGENT_NARRATE publish（5 种 event_kind 各验）
 * - 无效 JSON / schema 不符 → rejectedContract++，不 publish
 * - appeared 叙事 scope="zone"，target=zone_name
 * - 死亡广播叙事 scope="broadcast"
 * - connect/disconnect 生命周期
 * - stats 初始化 / 计数器自增
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { CHANNELS } from "@bong/schema";

import {
  ElderEncounterNarrationRuntime,
  renderAppearedNarration,
  renderDanReceivedNarration,
  renderDeathBroadcast,
} from "./elder-encounter-narration.js";

const { ELDER_ENCOUNTER, AGENT_NARRATE } = CHANNELS;

// ──── mock client ────────────────────────────────────────────────────────────

function makeMockClient() {
  const listeners: Map<string, ((ch: string, msg: string) => void)[]> = new Map();

  return {
    subscribe: vi.fn(async (_channel: string) => {}),
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
    publish: vi.fn(async (_channel: string, _message: string) => 1),

    emit(channel: string, message: string) {
      (listeners.get("message") ?? []).forEach((l) => l(channel, message));
    },
  };
}

// ──── payload helpers ────────────────────────────────────────────────────────

type EventKind =
  | "appeared"
  | "dan_received"
  | "betrayal"
  | "dead_natural"
  | "dead_player_kill";

function makePayload(eventKind: EventKind, overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    zone_name: "tsy_deep",
    elder_entity_id: 42,
    event_kind: eventKind,
    betray_probability: eventKind === "appeared" ? 0.65 : 0.0,
    dan_count: eventKind === "dan_received" ? 3 : 0,
    offered_skill_id: eventKind === "appeared" ? "woliu.heart" : "",
    // Bug4 修复：schema 新增 qi_fraction 字段（server 必发，additionalProperties:false 要求）
    qi_fraction: ["betrayal", "dead_natural", "dead_player_kill"].includes(eventKind) ? 0.0 : 1.0,
    server_tick: 720000,
    ...overrides,
  });
}

// ──── tests ──────────────────────────────────────────────────────────────────

describe("ElderEncounterNarrationRuntime", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let runtime: ElderEncounterNarrationRuntime;

  beforeEach(async () => {
    sub = makeMockClient();
    pub = makeMockClient();
    runtime = new ElderEncounterNarrationRuntime({ sub, pub });
    await runtime.connect();
  });

  it("connect 订阅 CHANNELS.ELDER_ENCOUNTER", () => {
    expect(
      sub.subscribe,
      `connect() 应订阅 ${ELDER_ENCOUNTER} 频道以接收遭遇事件`,
    ).toHaveBeenCalledWith(ELDER_ENCOUNTER);
  });

  it("stats 初始为 0", () => {
    const r2 = new ElderEncounterNarrationRuntime({ sub, pub });
    expect(r2.stats).toEqual({
      received: 0,
      published: 0,
      rejectedContract: 0,
      ignored: 0,
    });
  });

  // ── appeared 事件 ──────────────────────────────────────────────────────────

  it("appeared → 产出 AGENT_NARRATE，scope=zone，target=zone_name", async () => {
    await runtime.handlePayload(makePayload("appeared"));
    expect(pub.publish, "appeared 事件应触发恰好一次 publish").toHaveBeenCalledOnce();
    const [channel, msg] = pub.publish.mock.calls[0] as [string, string];
    expect(channel, `叙事应发往 ${AGENT_NARRATE}`).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(msg) as { v: number; narrations: Array<{ scope: string; target: string }> };
    expect(envelope.v).toBe(1);
    expect(envelope.narrations).toHaveLength(1);
    expect(envelope.narrations[0]?.scope, "appeared 叙事 scope 应为 zone").toBe("zone");
    expect(
      envelope.narrations[0]?.target,
      "appeared 叙事 target 应为 zone_name",
    ).toBe("tsy_deep");
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
  });

  // ── dan_received 事件 ──────────────────────────────────────────────────────

  it("dan_received → 产出 zone scope 叙事", async () => {
    await runtime.handlePayload(makePayload("dan_received"));
    expect(pub.publish).toHaveBeenCalledOnce();
    const [channel, msg] = pub.publish.mock.calls[0] as [string, string];
    expect(channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string }> };
    expect(envelope.narrations[0]?.scope, "dan_received 叙事 scope 应为 zone").toBe("zone");
  });

  it("dan_received 叙事文本包含 dan_count", async () => {
    await runtime.handlePayload(makePayload("dan_received", { dan_count: 3 }));
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ text: string }> };
    const text = envelope.narrations[0]?.text ?? "";
    expect(text, "叙事应包含给丹数量 '3'").toContain("3");
  });

  // ── betrayal 事件 ──────────────────────────────────────────────────────────

  it("betrayal → 产出 broadcast scope 叙事", async () => {
    await runtime.handlePayload(makePayload("betrayal"));
    expect(pub.publish).toHaveBeenCalledOnce();
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string }> };
    expect(
      envelope.narrations[0]?.scope,
      "betrayal 死亡广播 scope 应为 broadcast",
    ).toBe("broadcast");
    expect(runtime.stats.received).toBe(1);
  });

  it("betrayal 叙事 kind 为 dying_elder_betrayal", async () => {
    await runtime.handlePayload(makePayload("betrayal"));
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ kind: string }> };
    expect(envelope.narrations[0]?.kind).toBe("dying_elder_betrayal");
  });

  // ── dead_natural 事件 ──────────────────────────────────────────────────────

  it("dead_natural → 产出 broadcast scope 叙事", async () => {
    await runtime.handlePayload(makePayload("dead_natural"));
    expect(pub.publish).toHaveBeenCalledOnce();
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string; kind: string }> };
    expect(envelope.narrations[0]?.scope).toBe("broadcast");
    expect(envelope.narrations[0]?.kind).toBe("dying_elder_dead_natural");
  });

  // ── dead_player_kill 事件 ──────────────────────────────────────────────────

  it("dead_player_kill → 产出 broadcast scope 叙事", async () => {
    await runtime.handlePayload(makePayload("dead_player_kill"));
    expect(pub.publish).toHaveBeenCalledOnce();
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string; kind: string }> };
    expect(envelope.narrations[0]?.scope).toBe("broadcast");
    expect(envelope.narrations[0]?.kind).toBe("dying_elder_dead_player_kill");
  });

  // ── 错误路径 ───────────────────────────────────────────────────────────────

  it("无效 JSON → rejectedContract++，不 publish", async () => {
    await runtime.handlePayload("not-json");
    expect(pub.publish, "非法 JSON 不应触发 publish").not.toHaveBeenCalled();
    expect(
      runtime.stats.rejectedContract,
      "无效 JSON 应令 rejectedContract=1",
    ).toBe(1);
  });

  it("schema 不符（缺 zone_name）→ rejectedContract++，不 publish", async () => {
    const bad = JSON.stringify({
      elder_entity_id: 1,
      event_kind: "appeared",
      betray_probability: 0.5,
      dan_count: 0,
      offered_skill_id: "",
      server_tick: 100,
      // zone_name 缺失
    });
    await runtime.handlePayload(bad);
    expect(pub.publish, "缺 zone_name 的 payload 应被 schema 校验拒绝").not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("schema 不符（event_kind 非法值）→ rejectedContract++，不 publish", async () => {
    const bad = JSON.stringify({
      zone_name: "tsy_deep",
      elder_entity_id: 1,
      event_kind: "unknown_kind",
      betray_probability: 0.5,
      dan_count: 0,
      offered_skill_id: "",
      server_tick: 100,
    });
    await runtime.handlePayload(bad);
    expect(pub.publish).not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("非 ELDER_ENCOUNTER channel 消息通过 onMessage 路由被忽略", async () => {
    sub.emit("bong:other_channel", makePayload("appeared"));
    await new Promise((r) => setTimeout(r, 0));
    expect(pub.publish, "其他 channel 消息不应触发 publish").not.toHaveBeenCalled();
  });

  it("ELDER_ENCOUNTER channel 消息通过 onMessage emit 走完整路径", async () => {
    sub.emit(ELDER_ENCOUNTER, makePayload("appeared"));
    await new Promise((r) => setTimeout(r, 0));
    expect(pub.publish, "ELDER_ENCOUNTER emit 应触发 publish").toHaveBeenCalledOnce();
  });

  // ── disconnect ─────────────────────────────────────────────────────────────

  it("disconnect 清理 listener + unsubscribe", async () => {
    await runtime.disconnect();
    expect(
      sub.off,
      "disconnect() 应调用 sub.off 移除 onMessage 监听器",
    ).toHaveBeenCalled();
    expect(
      sub.unsubscribe,
      "disconnect() 应调用 sub.unsubscribe 退订频道",
    ).toHaveBeenCalled();
  });

  it("disconnect 后消息不再被处理", async () => {
    await runtime.disconnect();
    sub.emit(ELDER_ENCOUNTER, makePayload("appeared"));
    await new Promise((r) => setTimeout(r, 0));
    expect(
      pub.publish,
      "disconnect() 后收到的消息不应触发任何 publish",
    ).not.toHaveBeenCalled();
  });
});

// ──── 纯函数叙事模板测试 ─────────────────────────────────────────────────────

describe("renderAppearedNarration", () => {
  it("scope 为 zone，target 为 zone_name", () => {
    const result = renderAppearedNarration({
      zone_name: "tsy_abyss",
      elder_entity_id: 5,
      event_kind: "appeared",
      betray_probability: 0.65,
      dan_count: 0,
      offered_skill_id: "woliu.heart",
      qi_fraction: 1.0,
      server_tick: 1000,
    });
    expect(result.scope).toBe("zone");
    expect(result.target).toBe("tsy_abyss");
    expect(result.kind).toBe("dying_elder_appeared");
    expect(typeof result.text).toBe("string");
    expect(result.text.length, "叙事文本不应为空").toBeGreaterThan(0);
  });

  it("高 betray_probability 叙事包含危险暗示", () => {
    const result = renderAppearedNarration({
      zone_name: "tsy_deep",
      elder_entity_id: 1,
      event_kind: "appeared",
      betray_probability: 0.9,
      dan_count: 0,
      offered_skill_id: "woliu.turbulence_burst",
      qi_fraction: 1.0,
      server_tick: 0,
    });
    expect(result.text, "高危险度应包含'险机'暗示").toContain("险机");
  });

  it("低 betray_probability 叙事不含'险机'", () => {
    const result = renderAppearedNarration({
      zone_name: "tsy_shallow",
      elder_entity_id: 2,
      event_kind: "appeared",
      betray_probability: 0.35,
      dan_count: 0,
      offered_skill_id: "anqi.echo_fractal",
      qi_fraction: 1.0,
      server_tick: 0,
    });
    expect(result.text, "低危险度不应包含'险机'").not.toContain("险机");
  });
});

describe("renderDanReceivedNarration", () => {
  it("scope 为 zone，text 包含 dan_count", () => {
    const result = renderDanReceivedNarration({
      zone_name: "tsy_deep",
      elder_entity_id: 10,
      event_kind: "dan_received",
      betray_probability: 0.0,
      dan_count: 4,
      offered_skill_id: "sword_path.heaven_gate",
      qi_fraction: 0.7,
      server_tick: 500,
    });
    expect(result.scope).toBe("zone");
    expect(result.kind).toBe("dying_elder_dan_received");
    expect(result.text).toContain("4");
  });
});

describe("renderDeathBroadcast", () => {
  it("betrayal → scope=broadcast, kind=dying_elder_betrayal", () => {
    const result = renderDeathBroadcast(
      {
        zone_name: "tsy_deep",
        elder_entity_id: 7,
        event_kind: "betrayal",
        betray_probability: 0.0,
        dan_count: 5,
        offered_skill_id: "",
        qi_fraction: 0.0,
        server_tick: 999,
      },
      "betrayal",
    );
    expect(result.scope).toBe("broadcast");
    expect(result.kind).toBe("dying_elder_betrayal");
    expect(result.text).toContain("tsy_deep");
  });

  it("dead_natural → scope=broadcast, kind=dying_elder_dead_natural", () => {
    const result = renderDeathBroadcast(
      {
        zone_name: "tsy_abyss",
        elder_entity_id: 3,
        event_kind: "dead_natural",
        betray_probability: 0.0,
        dan_count: 0,
        offered_skill_id: "",
        qi_fraction: 0.0,
        server_tick: 100,
      },
      "dead_natural",
    );
    expect(result.scope).toBe("broadcast");
    expect(result.kind).toBe("dying_elder_dead_natural");
  });

  it("dead_player_kill → scope=broadcast, kind=dying_elder_dead_player_kill", () => {
    const result = renderDeathBroadcast(
      {
        zone_name: "tsy_deep",
        elder_entity_id: 1,
        event_kind: "dead_player_kill",
        betray_probability: 0.0,
        dan_count: 0,
        offered_skill_id: "",
        qi_fraction: 0.0,
        server_tick: 200,
      },
      "dead_player_kill",
    );
    expect(result.scope).toBe("broadcast");
    expect(result.kind).toBe("dying_elder_dead_player_kill");
    expect(result.text, "死亡广播文本应提及 zone_name").toContain("tsy_deep");
  });

  it("target 格式为 zone:{zone_name}|elder:{idx}", () => {
    const result = renderDeathBroadcast(
      {
        zone_name: "tsy_test",
        elder_entity_id: 99,
        event_kind: "dead_natural",
        betray_probability: 0.0,
        dan_count: 0,
        offered_skill_id: "",
        qi_fraction: 0.0,
        server_tick: 0,
      },
      "dead_natural",
    );
    expect(result.target).toBe("zone:tsy_test|elder:99");
  });
});
