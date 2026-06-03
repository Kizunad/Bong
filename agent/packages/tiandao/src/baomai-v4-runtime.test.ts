/**
 * plan-combat-skill-feedback-bridges-v1 P1 — BaomaiV4NarrationRuntime 单测。
 *
 * 测契约：
 *   - 5 个 channel 各自收消息 → 产 AGENT_NARRATE
 *   - 无效 JSON / schema 不符 → rejectedContract++，不 publish
 *   - 未知 channel → ignored++，不 publish
 *   - cleanup fn（disconnect）存在且可调用
 */

import { beforeEach, describe, expect, it } from "vitest";

import { CHANNELS } from "@bong/schema";

import { BaomaiV4NarrationRuntime } from "./baomai-v4-runtime.js";

const {
  AGENT_NARRATE,
  BAOMAI_V4_SCAR_CIRCUIT_FORMED,
  BAOMAI_V4_SCAR_CIRCUIT_BROKEN,
  BAOMAI_V4_IRON_COCOON_STAGE_UP,
  BAOMAI_V4_RESONANCE_LOCK,
  BAOMAI_V4_RESONANCE_LOCK_END,
} = CHANNELS;

// ──── mock client ─────────────────────────────────────────────────────────────

function makeMockClient() {
  const listeners: Map<string, ((pattern: string, ch: string, msg: string) => void)[]> = new Map();

  return {
    subscribed: [] as string[],
    published: [] as { channel: string; message: string }[],
    disconnected: false,

    psubscribe: async (_pattern: string) => {
      /* no-op */
    },
    on: (event: string, listener: (pattern: string, ch: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      arr.push(listener);
      listeners.set(event, arr);
    },
    off: (event: string, listener: (pattern: string, ch: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      const idx = arr.indexOf(listener);
      if (idx !== -1) arr.splice(idx, 1);
      listeners.set(event, arr);
    },
    punsubscribe: async () => {
      /* no-op */
    },
    disconnect: () => {
      /* no-op */
    },
    publish: async (channel: string, message: string) => {
      return 1;
    },

    // 触发 pmessage 事件（模拟 Redis psubscribe pmessage）
    emitPMessage(pattern: string, channel: string, message: string) {
      (listeners.get("pmessage") ?? []).forEach((l) => l(pattern, channel, message));
    },
  };
}

// ──── sample payloads ─────────────────────────────────────────────────────────

function makeScarCircuitFormedPayload(): string {
  return JSON.stringify({
    v: 1,
    entity_id: "offline:TestPlayer",
    circuit: "tiger_mouth",
    tick: 100,
  });
}

function makeScarCircuitBrokenPayload(): string {
  return JSON.stringify({
    v: 1,
    entity_id: "offline:TestPlayer",
    circuit: "heart_lung",
    reason: "healed",
    tick: 200,
  });
}

function makeIronCocoonStageUpPayload(): string {
  return JSON.stringify({
    v: 1,
    entity_id: "offline:TestPlayer",
    from: "none",
    to: "tough_skin",
    total_overloads: 50,
    tick: 300,
  });
}

function makeResonanceLockPayload(): string {
  return JSON.stringify({
    v: 1,
    fighter_a_id: "offline:PlayerA",
    fighter_b_id: "offline:PlayerB",
    started_at: 1000,
    ends_at: 1060,
  });
}

function makeResonanceLockEndExpiredPayload(): string {
  return JSON.stringify({
    v: 1,
    fighter_a_id: "offline:PlayerA",
    fighter_b_id: "offline:PlayerB",
    reason: { type: "expired" },
    tick: 1060,
  });
}

// ──── tests ───────────────────────────────────────────────────────────────────

describe("BaomaiV4NarrationRuntime", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let runtime: BaomaiV4NarrationRuntime;

  beforeEach(async () => {
    sub = makeMockClient();
    pub = makeMockClient();
    runtime = new BaomaiV4NarrationRuntime({ sub, pub });
    await runtime.connect();
  });

  // ── connect ──────────────────────────────────────────────────────────────

  it("stats initialized to zero", () => {
    const r2 = new BaomaiV4NarrationRuntime({ sub, pub });
    expect(r2.stats).toEqual({
      received: 0,
      ignored: 0,
      published: 0,
      rejectedContract: 0,
    });
  });

  // ── scar_circuit_formed ───────────────────────────────────────────────────

  it("scar_circuit_formed → AGENT_NARRATE published", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_SCAR_CIRCUIT_FORMED, makeScarCircuitFormedPayload());
    expect(
      spy,
      `scar_circuit_formed 应触发 publish 到 ${AGENT_NARRATE}`,
    ).toHaveBeenCalledOnce();
    const [channel, msg] = spy.mock.calls[0] as [string, string];
    expect(channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(msg) as { v: number; narrations: unknown[] };
    expect(envelope.v).toBe(1);
    expect(envelope.narrations).toHaveLength(1);
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
  });

  it("scar_circuit_formed narration uses perception style", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_SCAR_CIRCUIT_FORMED, makeScarCircuitFormedPayload());
    const [, msg] = spy.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ style: string }> };
    expect(
      envelope.narrations[0]?.style,
      "scar_circuit_formed 叙事应使用 perception 风格",
    ).toBe("perception");
  });

  // ── scar_circuit_broken ───────────────────────────────────────────────────

  it("scar_circuit_broken → AGENT_NARRATE published", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_SCAR_CIRCUIT_BROKEN, makeScarCircuitBrokenPayload());
    expect(spy).toHaveBeenCalledOnce();
    const [channel] = spy.mock.calls[0] as [string, string];
    expect(channel).toBe(AGENT_NARRATE);
    expect(runtime.stats.published).toBe(1);
  });

  it("all 4 break reasons produce narration", async () => {
    const reasons = ["healed", "deepened", "severed", "closed"];
    for (const reason of reasons) {
      sub = makeMockClient();
      pub = makeMockClient();
      const r = new BaomaiV4NarrationRuntime({ sub, pub });
      await r.connect();
      const spy = vi.spyOn(pub, "publish");
      await r.handleMessage(
        BAOMAI_V4_SCAR_CIRCUIT_BROKEN,
        JSON.stringify({
          v: 1,
          entity_id: "offline:TestPlayer",
          circuit: "liver_kidney",
          reason,
          tick: 50,
        }),
      );
      expect(spy, `reason '${reason}' should produce narration`).toHaveBeenCalledOnce();
    }
  });

  // ── iron_cocoon_stage_up ──────────────────────────────────────────────────

  it("iron_cocoon_stage_up → AGENT_NARRATE published", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_IRON_COCOON_STAGE_UP, makeIronCocoonStageUpPayload());
    expect(spy).toHaveBeenCalledOnce();
    const [channel] = spy.mock.calls[0] as [string, string];
    expect(channel).toBe(AGENT_NARRATE);
    expect(runtime.stats.published).toBe(1);
  });

  it("iron_cocoon_stage_up narration uses narration style", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_IRON_COCOON_STAGE_UP, makeIronCocoonStageUpPayload());
    const [, msg] = spy.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ style: string }> };
    expect(
      envelope.narrations[0]?.style,
      "iron_cocoon_stage_up 叙事应使用 narration 风格",
    ).toBe("narration");
  });

  it("iron_cocoon_stage_up narration mentions total_overloads", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_IRON_COCOON_STAGE_UP, makeIronCocoonStageUpPayload());
    const [, msg] = spy.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ text: string }> };
    expect(envelope.narrations[0]?.text).toContain("50");
  });

  // ── resonance_lock ────────────────────────────────────────────────────────

  it("resonance_lock → AGENT_NARRATE published", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_RESONANCE_LOCK, makeResonanceLockPayload());
    expect(spy).toHaveBeenCalledOnce();
    const [channel] = spy.mock.calls[0] as [string, string];
    expect(channel).toBe(AGENT_NARRATE);
    expect(runtime.stats.published).toBe(1);
  });

  it("resonance_lock narration scope is global", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_RESONANCE_LOCK, makeResonanceLockPayload());
    const [, msg] = spy.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string }> };
    expect(
      envelope.narrations[0]?.scope,
      "resonance_lock 叙事应为 broadcast scope（双方玩家都需收到）",
    ).toBe("broadcast");
  });

  // ── resonance_lock_end ────────────────────────────────────────────────────

  it("resonance_lock_end expired → AGENT_NARRATE published", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_RESONANCE_LOCK_END, makeResonanceLockEndExpiredPayload());
    expect(spy).toHaveBeenCalledOnce();
    expect(runtime.stats.published).toBe(1);
  });

  it("resonance_lock_end retreated → published", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(
      BAOMAI_V4_RESONANCE_LOCK_END,
      JSON.stringify({
        v: 1,
        fighter_a_id: "offline:PlayerA",
        fighter_b_id: "offline:PlayerB",
        reason: { type: "retreated", entity_id: "offline:PlayerA" },
        tick: 1030,
      }),
    );
    expect(spy).toHaveBeenCalledOnce();
  });

  it("resonance_lock_end death → published", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(
      BAOMAI_V4_RESONANCE_LOCK_END,
      JSON.stringify({
        v: 1,
        fighter_a_id: "offline:PlayerA",
        fighter_b_id: "offline:PlayerB",
        reason: { type: "death", entity_id: "offline:PlayerB" },
        tick: 1010,
      }),
    );
    expect(spy).toHaveBeenCalledOnce();
  });

  // ── error handling ────────────────────────────────────────────────────────

  it("invalid JSON → rejectedContract++, not published", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage(BAOMAI_V4_SCAR_CIRCUIT_FORMED, "not-json");
    expect(
      spy,
      "无效 JSON 不应触发 publish",
    ).not.toHaveBeenCalled();
    expect(
      runtime.stats.rejectedContract,
      "无效 JSON 应令 rejectedContract 自增为 1",
    ).toBe(1);
  });

  it("schema mismatch → rejectedContract++, not published", async () => {
    const spy = vi.spyOn(pub, "publish");
    const bad = JSON.stringify({ v: 1, entity_id: "offline:X" }); // missing circuit
    await runtime.handleMessage(BAOMAI_V4_SCAR_CIRCUIT_FORMED, bad);
    expect(spy).not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("unknown channel → ignored, not published", async () => {
    const spy = vi.spyOn(pub, "publish");
    await runtime.handleMessage("bong:baomai_v4/unknown_future_channel", makeScarCircuitFormedPayload());
    expect(spy).not.toHaveBeenCalled();
    expect(runtime.stats.ignored).toBe(1);
  });

  // ── pmessage routing ──────────────────────────────────────────────────────

  it("onPMessage via emitPMessage routes correctly", async () => {
    const spy = vi.spyOn(pub, "publish");
    sub.emitPMessage("bong:baomai_v4/*", BAOMAI_V4_RESONANCE_LOCK, makeResonanceLockPayload());
    await new Promise((r) => setTimeout(r, 0));
    expect(spy).toHaveBeenCalledOnce();
  });

  // ── disconnect ────────────────────────────────────────────────────────────

  it("disconnect stops processing", async () => {
    await runtime.disconnect();
    const spy = vi.spyOn(pub, "publish");
    // After disconnect, emitting should not produce narrations
    sub.emitPMessage("bong:baomai_v4/*", BAOMAI_V4_SCAR_CIRCUIT_FORMED, makeScarCircuitFormedPayload());
    await new Promise((r) => setTimeout(r, 0));
    expect(
      spy,
      "disconnect 后不应再 publish",
    ).not.toHaveBeenCalled();
  });
});

// import vitest globally (needed for vi.spyOn in beforeEach-based tests)
import { vi } from "vitest";
