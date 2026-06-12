/**
 * plan-halfstep-rechallenge-integration-v1 P1 — HalfStepRechallengeNarrationRuntime 单测
 *
 * 测契约：
 * - 收 bong:tribulation/halfstep_rechallenge 消息 → 产 AGENT_NARRATE publish（player scope 常规）
 * - zone_halfstep_count >= 2 → 额外产出 zone scope narration
 * - zone_halfstep_count < 2 → 不产 zone scope narration
 * - 无效 JSON / schema 不符 → rejectedContract++，不 publish
 * - connect 订阅正确频道
 * - disconnect 清理 listener + unsubscribe
 * - 非目标 channel 消息被忽略
 * - schema 正反 sample 对拍
 * - stats 初始化 / 计数器自增
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { CHANNELS } from "@bong/schema";

import {
  HalfStepRechallengeNarrationRuntime,
  renderPlayerNarration,
  renderZoneEchoNarration,
} from "./halfstep-rechallenge-narration.js";

const { HALFSTEP_RECHALLENGE, AGENT_NARRATE } = CHANNELS;

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

function makePayload(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    char_id: "player_001",
    zone_name: "qingyun_peaks",
    zone_halfstep_count: 1,
    at_tick: 12345,
    ...overrides,
  });
}

// ──── 主 Runtime 测试 ────────────────────────────────────────────────────────

describe("HalfStepRechallengeNarrationRuntime", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let runtime: HalfStepRechallengeNarrationRuntime;

  beforeEach(async () => {
    sub = makeMockClient();
    pub = makeMockClient();
    runtime = new HalfStepRechallengeNarrationRuntime({ sub, pub });
    await runtime.connect();
  });

  // ── connect ────────────────────────────────────────────────────────────────

  it("connect 订阅 CHANNELS.HALFSTEP_RECHALLENGE", () => {
    expect(
      sub.subscribe,
      `connect() 应订阅 ${HALFSTEP_RECHALLENGE} 以接收重渡触发事件`,
    ).toHaveBeenCalledWith(HALFSTEP_RECHALLENGE);
  });

  // ── stats ──────────────────────────────────────────────────────────────────

  it("stats 初始为 0", () => {
    const r2 = new HalfStepRechallengeNarrationRuntime({ sub, pub });
    expect(r2.stats, "新建 runtime stats 全字段应为 0").toEqual({
      received: 0,
      published: 0,
      rejectedContract: 0,
      ignored: 0,
    });
  });

  // ── player scope narration（常规路径）──────────────────────────────────────

  it("zone_halfstep_count=1 → 仅产 player scope narration", async () => {
    await runtime.handlePayload(makePayload({ zone_halfstep_count: 1 }));
    expect(pub.publish, "player scope 路径应触发恰好一次 publish").toHaveBeenCalledOnce();
    const [channel, msg] = pub.publish.mock.calls[0] as [string, string];
    expect(channel, `叙事应发往 ${AGENT_NARRATE}`).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(msg) as { v: number; narrations: Array<{ scope: string; target: string; text: string; style: string }> };
    expect(envelope.v, "NarrationV1 envelope 应包含 v:1").toBe(1);
    expect(envelope.narrations, "zone_halfstep_count=1 应恰好产 1 条叙事").toHaveLength(1);
    expect(envelope.narrations[0]?.scope, "单条叙事 scope 应为 player").toBe("player");
    expect(
      envelope.narrations[0]?.target,
      "player scope narration target 应为 char_id",
    ).toBe("player_001");
    expect(
      envelope.narrations[0]?.text,
      "player narration 文案应为 plan P1 确切串",
    ).toBe("你感到曾遭封压的经脉微微松动，或许时机已到。");
    expect(
      envelope.narrations[0]?.style,
      "player narration style 应为 perception（修士感知，非叙述者旁白）",
    ).toBe("perception");
    expect(runtime.stats.received, "处理后 received 应自增至 1").toBe(1);
    expect(runtime.stats.published, "处理后 published 应自增至 1").toBe(1);
  });

  // ── zone scope narration（zone_halfstep_count >= 2）────────────────────────

  it("zone_halfstep_count=2 → 产 player + zone scope 两条 narration", async () => {
    await runtime.handlePayload(makePayload({ zone_halfstep_count: 2 }));
    expect(pub.publish, "zone_halfstep_count=2 应触发恰好一次 publish（envelope 含两条）").toHaveBeenCalledOnce();
    const [channel, msg] = pub.publish.mock.calls[0] as [string, string];
    expect(channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string; target: string }> };
    expect(envelope.narrations, "zone_halfstep_count=2 应产 2 条叙事（player + zone）").toHaveLength(2);
    const scopes = envelope.narrations.map((n) => n.scope);
    expect(scopes, "应含 player scope").toContain("player");
    expect(scopes, "应含 zone scope").toContain("zone");
  });

  it("zone_halfstep_count=5 → zone scope target 为 zone_name，文案+style 锁定", async () => {
    await runtime.handlePayload(makePayload({ zone_halfstep_count: 5, zone_name: "spring_marsh" }));
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string; target: string; text: string; style: string }> };
    const zoneNarration = envelope.narrations.find((n) => n.scope === "zone");
    expect(zoneNarration, "zone scope narration 应存在").toBeDefined();
    expect(
      zoneNarration?.target,
      "zone scope narration target 应为 zone_name",
    ).toBe("spring_marsh");
    expect(
      zoneNarration?.text,
      "zone echo 文案应为 plan P1 确切串",
    ).toBe("虚空中某处的修士收到了相同的消息。");
    expect(
      zoneNarration?.style,
      "zone echo narration style 应为 perception",
    ).toBe("perception");
  });

  it("zone_halfstep_count=0 → 仅产 player scope，不产 zone scope", async () => {
    await runtime.handlePayload(makePayload({ zone_halfstep_count: 0 }));
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string }> };
    expect(envelope.narrations, "zone_halfstep_count=0 应仅产 1 条叙事").toHaveLength(1);
    expect(envelope.narrations[0]?.scope, "唯一叙事 scope 应为 player").toBe("player");
  });

  // ── dormant fallback ───────────────────────────────────────────────────────

  it("dormant NPC（zone_name='dormant', zone_halfstep_count=0）→ 仅产 player scope", async () => {
    await runtime.handlePayload(
      makePayload({ char_id: "npc_elder_42", zone_name: "dormant", zone_halfstep_count: 0 }),
    );
    expect(pub.publish).toHaveBeenCalledOnce();
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string; target: string }> };
    expect(envelope.narrations).toHaveLength(1);
    expect(envelope.narrations[0]?.scope).toBe("player");
    expect(envelope.narrations[0]?.target, "dormant NPC 的 target 应为 char_id").toBe("npc_elder_42");
  });

  // ── 错误路径 ───────────────────────────────────────────────────────────────

  it("无效 JSON → rejectedContract++，不 publish", async () => {
    await runtime.handlePayload("not-json{{");
    expect(pub.publish, "非法 JSON 不应触发 publish").not.toHaveBeenCalled();
    expect(
      runtime.stats.rejectedContract,
      "无效 JSON 应令 rejectedContract=1",
    ).toBe(1);
  });

  it("schema 不符（缺 char_id）→ rejectedContract++，不 publish", async () => {
    const bad = JSON.stringify({
      zone_name: "qingyun_peaks",
      zone_halfstep_count: 1,
      at_tick: 100,
      // char_id 缺失
    });
    await runtime.handlePayload(bad);
    expect(pub.publish, "缺 char_id 的 payload 应被 schema 校验拒绝").not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("schema 不符（zone_halfstep_count 为负数）→ rejectedContract++，不 publish", async () => {
    const bad = JSON.stringify({
      char_id: "player_001",
      zone_name: "qingyun_peaks",
      zone_halfstep_count: -1,
      at_tick: 100,
    });
    await runtime.handlePayload(bad);
    expect(pub.publish, "zone_halfstep_count 负数应被 schema 拒绝").not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("schema 不符（additionalProperties 字段）→ rejectedContract++，不 publish", async () => {
    const bad = JSON.stringify({
      char_id: "player_001",
      zone_name: "qingyun_peaks",
      zone_halfstep_count: 1,
      at_tick: 100,
      unknown_field: "should_be_rejected",
    });
    await runtime.handlePayload(bad);
    expect(pub.publish, "含 additionalProperties 的 payload 应被 schema 拒绝").not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  // ── channel 路由 ───────────────────────────────────────────────────────────

  it("非 HALFSTEP_RECHALLENGE channel 消息通过 onMessage 路由被忽略", async () => {
    sub.emit("bong:other_channel", makePayload());
    await new Promise((r) => setTimeout(r, 0));
    expect(pub.publish, "其他 channel 消息不应触发 publish").not.toHaveBeenCalled();
  });

  it("HALFSTEP_RECHALLENGE channel 消息通过 onMessage emit 走完整路径", async () => {
    sub.emit(HALFSTEP_RECHALLENGE, makePayload());
    await new Promise((r) => setTimeout(r, 0));
    expect(pub.publish, `${HALFSTEP_RECHALLENGE} emit 应触发 publish`).toHaveBeenCalledOnce();
  });

  // ── disconnect ─────────────────────────────────────────────────────────────

  it("disconnect 清理 listener + unsubscribe", async () => {
    await runtime.disconnect();
    expect(sub.off, "disconnect() 应调用 sub.off 移除 onMessage 监听器").toHaveBeenCalled();
    expect(sub.unsubscribe, "disconnect() 应调用 sub.unsubscribe 退订频道").toHaveBeenCalled();
  });
});

// ──── 纯函数叙事模板测试 ─────────────────────────────────────────────────────

describe("renderPlayerNarration", () => {
  it("scope 为 player，target 为 char_id", () => {
    const result = renderPlayerNarration({
      char_id: "cultivator_007",
      zone_name: "rift_valley",
      zone_halfstep_count: 1,
      at_tick: 9999,
    });
    expect(result.scope, "player scope narration 的 scope 应为 'player'").toBe("player");
    expect(result.target, "player scope narration 的 target 应为 char_id").toBe("cultivator_007");
  });

  it("文本精确等于 plan P1 确切串", () => {
    const result = renderPlayerNarration({
      char_id: "any",
      zone_name: "spawn",
      zone_halfstep_count: 0,
      at_tick: 0,
    });
    expect(
      result.text,
      "player narration 文案漂移检测：应精确等于 plan P1 规定的确切串",
    ).toBe("你感到曾遭封压的经脉微微松动，或许时机已到。");
  });

  it("style 严格为 perception", () => {
    const result = renderPlayerNarration({
      char_id: "any",
      zone_name: "spawn",
      zone_halfstep_count: 0,
      at_tick: 0,
    });
    expect(
      result.style,
      "player narration style 应为 perception（修士第一人称感知），而非叙述者旁白 narration",
    ).toBe("perception");
  });

  it("schema sample 正向对拍：合法 payload 产出符合 Narration 结构的对象", () => {
    const result = renderPlayerNarration({
      char_id: "sample_player",
      zone_name: "spawn",
      zone_halfstep_count: 0,
      at_tick: 1,
    });
    // scope / target / text / style 是 Narration 要求字段
    expect(result).toHaveProperty("scope", "player");
    expect(result).toHaveProperty("target", "sample_player");
    expect(result).toHaveProperty("text", "你感到曾遭封压的经脉微微松动，或许时机已到。");
    expect(result).toHaveProperty("style", "perception");
  });
});

describe("renderZoneEchoNarration", () => {
  it("scope 为 zone，target 为 zone_name", () => {
    const result = renderZoneEchoNarration({
      char_id: "npc_42",
      zone_name: "lingquan_marsh",
      zone_halfstep_count: 3,
      at_tick: 100,
    });
    expect(result.scope, "zone echo narration 的 scope 应为 'zone'").toBe("zone");
    expect(result.target, "zone echo narration 的 target 应为 zone_name").toBe("lingquan_marsh");
  });

  it("文案精确等于 plan P1 确切串", () => {
    const result = renderZoneEchoNarration({
      char_id: "any",
      zone_name: "north_wastes",
      zone_halfstep_count: 2,
      at_tick: 0,
    });
    expect(
      result.text,
      "zone echo 文案漂移检测：应精确等于 plan P1 规定的确切串",
    ).toBe("虚空中某处的修士收到了相同的消息。");
  });

  it("style 严格为 perception", () => {
    const result = renderZoneEchoNarration({
      char_id: "any",
      zone_name: "north_wastes",
      zone_halfstep_count: 2,
      at_tick: 0,
    });
    expect(
      result.style,
      "zone echo narration style 应为 perception（隐约感应，非直接告知），而非叙述者旁白 narration",
    ).toBe("perception");
  });
});
