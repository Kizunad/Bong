/**
 * plan-combat-skill-feedback-bridges-v1 P2 — BaomaiV3NarrationRuntime 单测。
 *
 * 测契约：
 *   - 5 个 channel（SKILL_EVENT + 4 新 channel）各自收消息 → 产 AGENT_NARRATE（routing table 测试）
 *   - 无效 JSON / schema 不符 → rejectedContract++，不 publish
 *   - 未知 channel → ignored++，不 publish
 *   - disperse flow_rate_multiplier=1.0（凡躯失败） → 走 else 分支文案
 *   - blood_burn ended_in_near_death=true → 特化文案
 *   - overload_ripple total_severity 3 档叙事
 */

import { beforeEach, describe, expect, it } from "vitest";

import { CHANNELS } from "@bong/schema";

import {
  BaomaiV3NarrationRuntime,
  renderBaomaiV3Narration,
  renderBloodBurnNarration,
  renderMountainShakeNarration,
  renderOverloadRippleNarration,
  renderTranscendenceExpiredNarration,
} from "./baomai-v3-runtime.js";

const {
  AGENT_NARRATE,
  BAOMAI_V3_SKILL_EVENT,
  BAOMAI_V3_MOUNTAIN_SHAKE,
  BAOMAI_V3_BLOOD_BURN,
  BAOMAI_V3_TRANSCENDENCE_EXPIRED,
  BAOMAI_V3_OVERLOAD_RIPPLE,
} = CHANNELS;

// ──── mock client ─────────────────────────────────────────────────────────────

function makeMockClient() {
  const listeners: Map<string, ((channel: string, msg: string) => void)[]> = new Map();

  return {
    subscribed: [] as string[],
    published: [] as { channel: string; message: string }[],

    subscribe: async (channel: string) => {
      /* no-op */
    },
    on: (event: string, listener: (channel: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      arr.push(listener);
      listeners.set(event, arr);
    },
    off: (event: string, listener: (channel: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      const idx = arr.indexOf(listener);
      if (idx !== -1) arr.splice(idx, 1);
      listeners.set(event, arr);
    },
    unsubscribe: async () => {
      /* no-op */
    },
    disconnect: () => {
      /* no-op */
    },
    publish: async (channel: string, message: string) => {
      listeners // keep reference happy
      return 1;
    },

    // 触发 message 事件（模拟 Redis subscribe message）
    emitMessage(channel: string, message: string) {
      (listeners.get("message") ?? []).forEach((l) => l(channel, message));
    },
  };
}

// ──── payload helpers ─────────────────────────────────────────────────────────

function makeSkillEventPayload(flowRateMultiplier = 10.0): string {
  return JSON.stringify({
    v: 1,
    type: "baomai_skill_event",
    skill_id: "disperse",
    caster_id: "offline:TestPlayer",
    tick: 42,
    qi_invested: 5350.0,
    damage: 0.0,
    blood_multiplier: 1.0,
    flow_rate_multiplier: flowRateMultiplier,
    meridian_ids: ["Ren", "Du"],
  });
}

function makeMountainShakePayload(affectedCount = 3): string {
  return JSON.stringify({
    v: 1,
    caster_id: "offline:TestPlayer",
    affected_count: affectedCount,
    tick: 200,
    qi_spent: 1200.0,
    radius_blocks: 5.0,
    shock_damage: 420.0,
  });
}

function makeBloodBurnPayload(endedInNearDeath = false): string {
  return JSON.stringify({
    v: 1,
    caster_id: "offline:TestPlayer",
    tick: 300,
    hp_burned: 150.0,
    qi_multiplier: 3.5,
    active_until_tick: 360,
    ended_in_near_death: endedInNearDeath,
  });
}

function makeTranscendenceExpiredPayload(): string {
  return JSON.stringify({
    v: 1,
    caster_id: "offline:TestPlayer",
    tick: 700,
  });
}

function makeOverloadRipplePayload(totalSeverity = 0.35): string {
  return JSON.stringify({
    v: 1,
    caster_id: "offline:TestPlayer",
    tick: 150,
    skill_id: "beng_quan",
    severity_delta: 0.05,
    total_severity: totalSeverity,
    meridian_ids: ["LargeIntestine", "Lung"],
  });
}

// ──── routing table tests ─────────────────────────────────────────────────────

describe("BaomaiV3NarrationRuntime — multi-channel routing table", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let runtime: BaomaiV3NarrationRuntime;

  beforeEach(async () => {
    sub = makeMockClient();
    pub = makeMockClient();

    // Capture pub.publish calls
    const pubMessages: { channel: string; message: string }[] = [];
    pub.publish = async (channel: string, message: string) => {
      pubMessages.push({ channel, message });
      return 1;
    };
    (pub as unknown as { messages: typeof pubMessages }).messages = pubMessages;

    runtime = new BaomaiV3NarrationRuntime({ sub, pub });
    await runtime.connect();
  });

  it("BAOMAI_V3_SKILL_EVENT route → publishes to AGENT_NARRATE", async () => {
    await runtime.handlePayload(BAOMAI_V3_SKILL_EVENT, makeSkillEventPayload(10.0));

    expect(runtime.stats.received, "received count should be 1").toBe(1);
    expect(runtime.stats.published, "published count should be 1").toBe(1);
    expect(runtime.stats.rejectedContract, "no contract rejection expected").toBe(0);
  });

  it("BAOMAI_V3_MOUNTAIN_SHAKE route → publishes to AGENT_NARRATE", async () => {
    await runtime.handlePayload(BAOMAI_V3_MOUNTAIN_SHAKE, makeMountainShakePayload(3));

    expect(runtime.stats.received, "mountain_shake received").toBe(1);
    expect(runtime.stats.published, "mountain_shake published").toBe(1);
  });

  it("BAOMAI_V3_BLOOD_BURN route → publishes to AGENT_NARRATE", async () => {
    await runtime.handlePayload(BAOMAI_V3_BLOOD_BURN, makeBloodBurnPayload(false));

    expect(runtime.stats.received, "blood_burn received").toBe(1);
    expect(runtime.stats.published, "blood_burn published").toBe(1);
  });

  it("BAOMAI_V3_TRANSCENDENCE_EXPIRED route → publishes to AGENT_NARRATE", async () => {
    await runtime.handlePayload(BAOMAI_V3_TRANSCENDENCE_EXPIRED, makeTranscendenceExpiredPayload());

    expect(runtime.stats.received, "transcendence_expired received").toBe(1);
    expect(runtime.stats.published, "transcendence_expired published").toBe(1);
  });

  it("BAOMAI_V3_OVERLOAD_RIPPLE route → publishes to AGENT_NARRATE", async () => {
    await runtime.handlePayload(BAOMAI_V3_OVERLOAD_RIPPLE, makeOverloadRipplePayload(0.35));

    expect(runtime.stats.received, "overload_ripple received").toBe(1);
    expect(runtime.stats.published, "overload_ripple published").toBe(1);
  });

  it("unknown channel → ignored, not published", async () => {
    await runtime.handlePayload("bong:unknown/channel", "{}");

    expect(runtime.stats.ignored, "unknown channel should increment ignored").toBe(1);
    expect(runtime.stats.received, "unknown channel should not increment received").toBe(0);
    expect(runtime.stats.published, "unknown channel should not publish").toBe(0);
  });

  it("invalid JSON → rejectedContract, not published", async () => {
    await runtime.handlePayload(BAOMAI_V3_SKILL_EVENT, "not-json{{");

    expect(runtime.stats.rejectedContract, "bad JSON should increment rejectedContract").toBe(1);
    expect(runtime.stats.published, "bad JSON should not publish").toBe(0);
  });

  it("schema mismatch → rejectedContract, not published", async () => {
    await runtime.handlePayload(
      BAOMAI_V3_SKILL_EVENT,
      JSON.stringify({ v: 1, type: "baomai_skill_event" /* missing required fields */ }),
    );

    expect(runtime.stats.rejectedContract, "schema mismatch should increment rejectedContract").toBeGreaterThanOrEqual(1);
    expect(runtime.stats.published, "schema mismatch should not publish").toBe(0);
  });

  it("onMessage listener routes correctly for each channel", async () => {
    // Simulate Redis delivering messages on each channel
    for (const [channel, payload] of [
      [BAOMAI_V3_SKILL_EVENT, makeSkillEventPayload()],
      [BAOMAI_V3_MOUNTAIN_SHAKE, makeMountainShakePayload()],
      [BAOMAI_V3_BLOOD_BURN, makeBloodBurnPayload()],
      [BAOMAI_V3_TRANSCENDENCE_EXPIRED, makeTranscendenceExpiredPayload()],
      [BAOMAI_V3_OVERLOAD_RIPPLE, makeOverloadRipplePayload()],
    ] as [string, string][]) {
      sub.emitMessage(channel, payload);
    }
    // Allow microtasks to flush
    await new Promise((resolve) => setImmediate(resolve));

    expect(
      runtime.stats.published,
      "all 5 channels should each produce one narration publish",
    ).toBe(5);
  });
});

// ──── [fix-major] mountain_shake / blood_burn 双重叙事去重 integration pin ────
//
// 一次 mountain_shake cast → 两条 channel 消息（skill_event + 专用事件）经
// onMessage 路由，最终恰好只产出 1 条 narration publish（来自专用渲染器）。
// blood_burn 同理。
// 锁住"单次 cast → 恰 1 条 narration"不发生 UX 回归（双重叙事）。

describe("integration: mountain_shake cast → exactly 1 narration (no double-narration)", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let runtime: BaomaiV3NarrationRuntime;
  let pubMessages: { channel: string; message: string }[];

  beforeEach(async () => {
    sub = makeMockClient();
    pub = makeMockClient();
    pubMessages = [];
    pub.publish = async (channel: string, message: string) => {
      pubMessages.push({ channel, message });
      return 1;
    };
    runtime = new BaomaiV3NarrationRuntime({ sub, pub });
    await runtime.connect();
  });

  it("mountain_shake: skill_event → null（不叙事），专用事件 → 1 条叙事，合计恰 1 条", async () => {
    // step 1: skill_event channel — skill_id=mountain_shake 应返回 null 不 publish
    const skillEventPayload = JSON.stringify({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "mountain_shake",
      caster_id: "offline:TestPlayer",
      tick: 500,
      qi_invested: 1200.0,
      damage: 0.0,
      blood_multiplier: 1.0,
      flow_rate_multiplier: 1.0,
      meridian_ids: ["Ren"],
    });
    sub.emitMessage(BAOMAI_V3_SKILL_EVENT, skillEventPayload);

    // step 2: dedicated mountain_shake channel — 应 publish 1 条
    sub.emitMessage(BAOMAI_V3_MOUNTAIN_SHAKE, makeMountainShakePayload(2));

    await new Promise((resolve) => setImmediate(resolve));

    expect(
      pubMessages.length,
      "一次 mountain_shake cast 经两条 channel 消息路由后，AGENT_NARRATE 应恰好收到 1 条 publish（来自专用渲染器，skill_event 对 mountain_shake 返回 null）",
    ).toBe(1);

    // 确认那 1 条来自专用渲染器（含命中数信息）
    const narrationBody = JSON.parse(pubMessages[0]!.message) as { v: number; narrations: { text: string }[] };
    expect(
      narrationBody.narrations[0]?.text,
      "专用叙事应含「掀翻」或「冲击」等山震专有词",
    ).toMatch(/掀翻|冲击|无人被抬乱/u);
  });

  it("blood_burn: skill_event → null（不叙事），专用事件 → 1 条叙事，合计恰 1 条", async () => {
    // step 1: skill_event channel — skill_id=blood_burn 应返回 null 不 publish
    const skillEventPayload = JSON.stringify({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "blood_burn",
      caster_id: "offline:TestPlayer",
      tick: 600,
      qi_invested: 0.0,
      damage: 0.0,
      blood_multiplier: 2.0,
      flow_rate_multiplier: 1.0,
      meridian_ids: [],
    });
    sub.emitMessage(BAOMAI_V3_SKILL_EVENT, skillEventPayload);

    // step 2: dedicated blood_burn channel — 应 publish 1 条
    sub.emitMessage(BAOMAI_V3_BLOOD_BURN, makeBloodBurnPayload(false));

    await new Promise((resolve) => setImmediate(resolve));

    expect(
      pubMessages.length,
      "一次 blood_burn cast 经两条 channel 消息路由后，AGENT_NARRATE 应恰好收到 1 条 publish（来自专用渲染器，skill_event 对 blood_burn 返回 null）",
    ).toBe(1);

    // 确认那 1 条来自专用渲染器（含 qi_multiplier 信息）
    const narrationBody = JSON.parse(pubMessages[0]!.message) as { v: number; narrations: { text: string }[] };
    expect(
      narrationBody.narrations[0]?.text,
      "专用叙事应含「膨涨」或「血量」等血燃专有词",
    ).toMatch(/膨涨|血量|腥气|濒死/u);
  });

  it("renderBaomaiV3Narration: mountain_shake skill_event → null", () => {
    const result = renderBaomaiV3Narration({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "mountain_shake",
      caster_id: "offline:TestPlayer",
      tick: 200,
      qi_invested: 1200.0,
      damage: 0.0,
      blood_multiplier: 1.0,
      flow_rate_multiplier: 1.0,
      meridian_ids: [],
    });
    expect(
      result,
      "skill_event channel 上 mountain_shake 必须返回 null（叙事权已交给专用渲染器）",
    ).toBeNull();
  });

  it("renderBaomaiV3Narration: blood_burn skill_event → null", () => {
    const result = renderBaomaiV3Narration({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "blood_burn",
      caster_id: "offline:TestPlayer",
      tick: 600,
      qi_invested: 0.0,
      damage: 0.0,
      blood_multiplier: 2.0,
      flow_rate_multiplier: 1.0,
      meridian_ids: [],
    });
    expect(
      result,
      "skill_event channel 上 blood_burn 必须返回 null（叙事权已交给专用渲染器）",
    ).toBeNull();
  });
});

// ──── disperse-failed pin test ────────────────────────────────────────────────

describe("renderBaomaiV3Narration — disperse flow_rate_multiplier pin", () => {
  it("flow_rate_multiplier=1.0 (凡躯失败) → else 分支文案（强行散功）", () => {
    const result = renderBaomaiV3Narration({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "disperse",
      caster_id: "offline:TestPlayer",
      tick: 100,
      qi_invested: 100.0,
      damage: 0.0,
      blood_multiplier: 1.0,
      flow_rate_multiplier: 1.0, // 非超越档 physics 返回值，schema minimum:1
      meridian_ids: [],
    });
    expect(result, "凡躯散功必须产出叙事（非 null）").not.toBeNull();
    expect(
      result?.text,
      "flow_rate_multiplier<10 应走 else 分支：「强行散功，凡躯没有应声」",
    ).toContain("强行散功");
  });

  it("flow_rate_multiplier=10.0 (超越档) → transcendence 分支文案", () => {
    const result = renderBaomaiV3Narration({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "disperse",
      caster_id: "offline:TestPlayer",
      tick: 100,
      qi_invested: 5350.0,
      damage: 0.0,
      blood_multiplier: 1.0,
      flow_rate_multiplier: 10.0,
      meridian_ids: ["Ren"],
    });
    expect(result?.text).toContain("烧去半池真元重铸凡躯");
  });
});

// ──── blood_burn near_death branch ───────────────────────────────────────────

describe("renderBloodBurnNarration — near death branch", () => {
  it("ended_in_near_death=true → 特化濒死文案", () => {
    const result = renderBloodBurnNarration({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 300,
      hp_burned: 300.0,
      qi_multiplier: 5.0,
      active_until_tick: 300,
      ended_in_near_death: true,
    });
    expect(result, "near death blood_burn 必须产叙事").not.toBeNull();
    expect(result?.text, "near death 文案应含「濒死」").toContain("濒死");
  });

  it("ended_in_near_death=false → 普通血燃文案（含 qi_multiplier）", () => {
    const result = renderBloodBurnNarration({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 300,
      hp_burned: 150.0,
      qi_multiplier: 3.5,
      active_until_tick: 360,
      ended_in_near_death: false,
    });
    expect(result, "normal blood_burn 必须产叙事").not.toBeNull();
    expect(result?.text, "normal 文案应含 qi_multiplier 值").toContain("3.5");
  });
});

// ──── overload_ripple severity 3 档 ──────────────────────────────────────────

describe("renderOverloadRippleNarration — total_severity 3 tier", () => {
  it("total_severity=0.85 → 危机档文案（距断脉）", () => {
    const result = renderOverloadRippleNarration({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 150,
      skill_id: "beng_quan",
      severity_delta: 0.05,
      total_severity: 0.85,
      meridian_ids: ["LargeIntestine"],
    });
    expect(result?.text, "危机档文案应含「断脉」").toContain("断脉");
  });

  it("total_severity=0.55 → 警示档文案（外溢）", () => {
    const result = renderOverloadRippleNarration({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 150,
      skill_id: "mountain_shake",
      severity_delta: 0.04,
      total_severity: 0.55,
      meridian_ids: ["Stomach"],
    });
    expect(result?.text, "警示档文案应含「外溢」").toContain("外溢");
  });

  it("total_severity=0.2 → 轻微档文案（可控）", () => {
    const result = renderOverloadRippleNarration({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 150,
      skill_id: "beng_quan",
      severity_delta: 0.05,
      total_severity: 0.2,
      meridian_ids: ["Lung"],
    });
    expect(result?.text, "轻微档文案应含「可控」").toContain("可控");
  });
});

// ──── mountain_shake affected_count variants ──────────────────────────────────

describe("renderMountainShakeNarration — affected_count variants", () => {
  it("affected_count=0 → 无人被抬乱文案", () => {
    const result = renderMountainShakeNarration({
      v: 1,
      caster_id: "offline:TestPlayer",
      affected_count: 0,
      tick: 200,
      qi_spent: 800.0,
      radius_blocks: 3.0,
      shock_damage: 250.0,
    });
    expect(result?.text).toContain("无人被抬乱");
  });

  it("affected_count=1 → 单人文案", () => {
    const result = renderMountainShakeNarration({
      v: 1,
      caster_id: "offline:TestPlayer",
      affected_count: 1,
      tick: 200,
      qi_spent: 800.0,
      radius_blocks: 3.0,
      shock_damage: 250.0,
    });
    expect(result?.text).toContain("一人");
  });

  it("affected_count=5 → 多人含半径文案", () => {
    const result = renderMountainShakeNarration({
      v: 1,
      caster_id: "offline:TestPlayer",
      affected_count: 5,
      tick: 200,
      qi_spent: 1200.0,
      radius_blocks: 5.0,
      shock_damage: 420.0,
    });
    expect(result?.text).toContain("5");
  });
});

// ──── transcendence_expired ───────────────────────────────────────────────────

describe("renderTranscendenceExpiredNarration", () => {
  it("produces narration containing actor name", () => {
    const result = renderTranscendenceExpiredNarration({
      v: 1,
      caster_id: "offline:TestPlayer",
      tick: 700,
    });
    expect(result, "transcendence_expired 必须产叙事").not.toBeNull();
    expect(result?.text).toContain("TestPlayer");
    expect(result?.text).toContain("窗口");
  });
});
