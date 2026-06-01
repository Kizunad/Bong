/**
 * plan-offscreen-war-v1 P9：war-outcome-narration.ts 饱和测试。
 *
 * 覆盖：
 * - settling 事件 → emit 1 条 broadcast narration，kind="war_outcome"，含 region_descriptor，无具名宗门
 * - skirmish/emerging/aftermath → 零 narration（仅 settling 播报）
 * - 模板 A vs B 选择分支（按 total_casualties / winner 判定）
 * - invalid payload（schema 校验失败）→ 零 narration + 不抛
 * - reframe b：零宗门名（文本中不含"青云"/"盟"等具名宗门字样）
 */

import { describe, expect, it, vi } from "vitest";
import { CHANNELS, type Narration, validateNarrationV1Contract } from "@bong/schema";

import {
  WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD,
  WarOutcomeNarrationRuntime,
  type WarOutcomeNarrationRuntimeClient,
  renderWarOutcomeNarration,
  renderWarOutcomeText,
  selectWarOutcomeTemplate,
} from "../src/war-outcome-narration.js";

const { AGENT_NARRATE, FACTION_WAR } = CHANNELS;

// ─────────────── FakePubSub ──────────────────────────────────────────────────

class FakePubSub implements WarOutcomeNarrationRuntimeClient {
  public published: Array<{ channel: string; message: string }> = [];
  public subscribedChannels: string[] = [];
  public listeners: Array<(channel: string, message: string) => void> = [];

  async subscribe(channel: string): Promise<void> {
    this.subscribedChannels.push(channel);
  }

  on(_event: string, listener: (channel: string, message: string) => void) {
    this.listeners.push(listener);
    return this;
  }

  off(_event: string, listener: (channel: string, message: string) => void) {
    this.listeners = this.listeners.filter((entry) => entry !== listener);
    return this;
  }

  async unsubscribe(): Promise<void> {}

  disconnect(): void {}

  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    return 1;
  }

  emit(channel: string, message: string): void {
    for (const listener of [...this.listeners]) {
      listener(channel, message);
    }
  }
}

const silent = { info: vi.fn(), warn: vi.fn() };

/** 让 fire-and-forget 的 publish（含内部 await publish）跑完。 */
function settleMicrotasks(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

// ─────────────── fixtures ────────────────────────────────────────────────────

function makeSettlingEvent(overrides: Record<string, unknown> = {}) {
  return {
    v: 1,
    kind: "faction_war_event",
    war_id: 7,
    zone: "残灰谷",
    region_descriptor: "残灰谷一带散修",
    phase: "settling",
    groups: [0, 1],
    enlist_count: 2,
    mercenary_count: 0,
    intercept_count: 1,
    spectate_count: 0,
    winner_group: 0,
    loser_group: 1,
    total_casualties: 8,
    at_tick: 10000,
    ...overrides,
  };
}

function makePhaseEvent(phase: string) {
  return {
    v: 1,
    kind: "faction_war_event",
    war_id: 3,
    zone: "残灰谷",
    region_descriptor: "残灰谷一带散修",
    phase,
    groups: [0, 1],
    enlist_count: 0,
    mercenary_count: 0,
    intercept_count: 0,
    spectate_count: 0,
    at_tick: 5000,
  };
}

// ─────────────── A. 纯函数单测 ───────────────────────────────────────────────

describe("selectWarOutcomeTemplate", () => {
  it("returns 'A' when winner exists and casualties < threshold", () => {
    const event = makeSettlingEvent({
      winner_group: 0,
      total_casualties: WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD - 1,
    });
    expect(selectWarOutcomeTemplate(event as Parameters<typeof selectWarOutcomeTemplate>[0])).toBe("A");
  });

  it("returns 'B' when casualties >= threshold (even with winner)", () => {
    const event = makeSettlingEvent({
      winner_group: 0,
      total_casualties: WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD,
    });
    expect(selectWarOutcomeTemplate(event as Parameters<typeof selectWarOutcomeTemplate>[0])).toBe("B");
  });

  it("returns 'B' when winner_group is absent (no outcome)", () => {
    const { winner_group, loser_group, total_casualties, ...noOutcome } = makeSettlingEvent();
    expect(
      selectWarOutcomeTemplate(noOutcome as Parameters<typeof selectWarOutcomeTemplate>[0]),
    ).toBe("B");
  });

  it("returns 'B' when casualties exactly at threshold", () => {
    const event = makeSettlingEvent({
      winner_group: 0,
      total_casualties: WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD,
    });
    expect(selectWarOutcomeTemplate(event as Parameters<typeof selectWarOutcomeTemplate>[0])).toBe("B");
  });
});

describe("renderWarOutcomeText", () => {
  it("template A contains region_descriptor", () => {
    const event = makeSettlingEvent({ total_casualties: 1, winner_group: 0 });
    const text = renderWarOutcomeText(event as Parameters<typeof renderWarOutcomeText>[0]);
    expect(text).toContain("残灰谷一带散修");
  });

  it("template B contains region_descriptor", () => {
    const event = makeSettlingEvent({ total_casualties: WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD });
    const text = renderWarOutcomeText(event as Parameters<typeof renderWarOutcomeText>[0]);
    expect(text).toContain("残灰谷一带散修");
  });

  it("reframe b: text does not contain any named sect (template A)", () => {
    const event = makeSettlingEvent({ total_casualties: 1, winner_group: 0 });
    const text = renderWarOutcomeText(event as Parameters<typeof renderWarOutcomeText>[0]);
    // 不应含任何具名宗门字样（青云、盟、宗主等）
    expect(text).not.toMatch(/青云|盟|宗主|门派|宗门/);
    // 应含「散修」
    expect(text).toContain("散修");
  });

  it("reframe b: text does not contain any named sect (template B)", () => {
    const event = makeSettlingEvent({ total_casualties: WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD });
    const text = renderWarOutcomeText(event as Parameters<typeof renderWarOutcomeText>[0]);
    expect(text).not.toMatch(/青云|盟|宗主|门派|宗门/);
    expect(text).toContain("散修");
  });

  it("template A and B produce distinct texts", () => {
    const eventA = makeSettlingEvent({ total_casualties: 1, winner_group: 0 });
    const eventB = makeSettlingEvent({ total_casualties: WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD });
    expect(
      renderWarOutcomeText(eventA as Parameters<typeof renderWarOutcomeText>[0]),
    ).not.toBe(renderWarOutcomeText(eventB as Parameters<typeof renderWarOutcomeText>[0]));
  });
});

describe("renderWarOutcomeNarration", () => {
  it("kind=war_outcome, scope=broadcast, style=perception", () => {
    const event = makeSettlingEvent();
    const narration = renderWarOutcomeNarration(event as Parameters<typeof renderWarOutcomeNarration>[0]);
    expect(narration.kind).toBe("war_outcome");
    expect(narration.scope).toBe("broadcast");
    expect(narration.style).toBe("perception");
  });

  it("passes NarrationV1 contract (wrapped in envelope)", () => {
    const event = makeSettlingEvent();
    const narration = renderWarOutcomeNarration(event as Parameters<typeof renderWarOutcomeNarration>[0]);
    const envelope = { v: 1 as const, narrations: [narration] };
    const result = validateNarrationV1Contract(envelope);
    expect(result.ok, `narration envelope should pass NarrationV1 contract, errors: ${result.errors.join("; ")}`).toBe(
      true,
    );
  });

  it("text contains region_descriptor (透传检查)", () => {
    const event = makeSettlingEvent({ region_descriptor: "血谷一带散修" });
    const narration = renderWarOutcomeNarration(event as Parameters<typeof renderWarOutcomeNarration>[0]);
    expect(narration.text).toContain("血谷一带散修");
  });
});

// ─────────── B. Runtime 集成测试 ──────────────────────────────────────────────

async function makeConnectedRuntime() {
  const sub = new FakePubSub();
  const pub = new FakePubSub();
  const runtime = new WarOutcomeNarrationRuntime({ sub, pub, logger: silent });
  await runtime.connect();
  return { sub, pub, runtime };
}

describe("WarOutcomeNarrationRuntime — settling emits narration", () => {
  it("settling event emits 1 broadcast war_outcome narration on AGENT_NARRATE", async () => {
    const { sub, pub, runtime } = await makeConnectedRuntime();

    sub.emit(FACTION_WAR, JSON.stringify(makeSettlingEvent()));
    await settleMicrotasks();

    expect(pub.published).toHaveLength(1);
    const { channel, message } = pub.published[0];
    expect(channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(message) as { v: number; narrations: Narration[] };
    expect(envelope["v"]).toBe(1);
    expect(Array.isArray(envelope["narrations"])).toBe(true);
    const narration = envelope["narrations"][0];
    expect(narration["kind"]).toBe("war_outcome");
    expect(narration["scope"]).toBe("broadcast");
    expect(String(narration["text"])).toContain("残灰谷一带散修");
    expect(runtime.stats.published).toBe(1);
    expect(runtime.stats.ignoredPhase).toBe(0);
    expect(runtime.stats.rejectedContract).toBe(0);
  });

  it("region_descriptor is transparently relayed (not a named sect)", async () => {
    const { sub, pub } = await makeConnectedRuntime();
    sub.emit(
      FACTION_WAR,
      JSON.stringify(makeSettlingEvent({ region_descriptor: "血谷一带散修" })),
    );
    await settleMicrotasks();
    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message) as { narrations: Narration[] };
    const text = String(envelope.narrations[0].text);
    expect(text).toContain("血谷一带散修");
    expect(text).not.toMatch(/青云|盟|宗主/);
  });
});

describe("WarOutcomeNarrationRuntime — non-settling phases emit nothing", () => {
  for (const phase of ["skirmish", "emerging", "aftermath"]) {
    it(`phase=${phase} emits zero narration`, async () => {
      const { sub, pub, runtime } = await makeConnectedRuntime();
      sub.emit(FACTION_WAR, JSON.stringify(makePhaseEvent(phase)));
      await settleMicrotasks();
      expect(pub.published).toHaveLength(0);
      expect(runtime.stats.ignoredPhase).toBe(1);
      expect(runtime.stats.published).toBe(0);
    });
  }
});

describe("WarOutcomeNarrationRuntime — invalid payload handling", () => {
  it("invalid JSON → zero narration + no throw", async () => {
    const { sub, pub, runtime } = await makeConnectedRuntime();
    sub.emit(FACTION_WAR, "not json {{{");
    await settleMicrotasks();
    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
    expect(runtime.stats.published).toBe(0);
  });

  it("valid JSON but bad schema (missing region_descriptor) → zero narration + rejected", async () => {
    const { sub, pub, runtime } = await makeConnectedRuntime();
    const badPayload = {
      v: 1,
      kind: "faction_war_event",
      war_id: 1,
      zone: "残灰谷",
      // region_descriptor missing
      phase: "settling",
      groups: [0, 1],
      enlist_count: 0,
      mercenary_count: 0,
      intercept_count: 0,
      spectate_count: 0,
      at_tick: 100,
    };
    sub.emit(FACTION_WAR, JSON.stringify(badPayload));
    await settleMicrotasks();
    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBeGreaterThan(0);
  });

  it("valid JSON but phase=invalid_phase → zero narration + rejected", async () => {
    const { sub, pub, runtime } = await makeConnectedRuntime();
    const badPhase = makeSettlingEvent({ phase: "invalid_phase" });
    sub.emit(FACTION_WAR, JSON.stringify(badPhase));
    await settleMicrotasks();
    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBeGreaterThan(0);
  });
});

describe("WarOutcomeNarrationRuntime — stats", () => {
  it("tracks received/published/ignored stats correctly", async () => {
    const { sub, pub, runtime } = await makeConnectedRuntime();

    // 1 settling (publish) + 1 skirmish (ignore) + 1 invalid (reject)
    sub.emit(FACTION_WAR, JSON.stringify(makeSettlingEvent()));
    sub.emit(FACTION_WAR, JSON.stringify(makePhaseEvent("skirmish")));
    sub.emit(FACTION_WAR, "bad json");
    await settleMicrotasks();

    expect(runtime.stats.received).toBe(3);
    expect(runtime.stats.published).toBe(1);
    expect(runtime.stats.ignoredPhase).toBe(1);
    expect(runtime.stats.rejectedContract).toBe(1);
  });
});

describe("WarOutcomeNarrationRuntime — template A vs B", () => {
  it("low casualties → template A text (mentions 一方势力)", async () => {
    const { sub, pub } = await makeConnectedRuntime();
    sub.emit(
      FACTION_WAR,
      JSON.stringify(makeSettlingEvent({ total_casualties: 1, winner_group: 0 })),
    );
    await settleMicrotasks();
    const envelope = JSON.parse(pub.published[0].message) as { narrations: Narration[] };
    const text = String(envelope.narrations[0].text);
    expect(text).toContain("一方势力");
  });

  it("high casualties → template B text (mentions 厮杀经久)", async () => {
    const { sub, pub } = await makeConnectedRuntime();
    sub.emit(
      FACTION_WAR,
      JSON.stringify(makeSettlingEvent({ total_casualties: WAR_OUTCOME_HIGH_CASUALTIES_THRESHOLD })),
    );
    await settleMicrotasks();
    const envelope = JSON.parse(pub.published[0].message) as { narrations: Narration[] };
    const text = String(envelope.narrations[0].text);
    expect(text).toContain("厮杀经久");
  });
});

describe("WarOutcomeNarrationRuntime — subscribed to correct channel", () => {
  it("subscribes to FACTION_WAR channel on connect", async () => {
    const sub = new FakePubSub();
    const pub = new FakePubSub();
    const runtime = new WarOutcomeNarrationRuntime({ sub, pub, logger: silent });
    await runtime.connect();
    expect(sub.subscribedChannels).toContain(FACTION_WAR);
    expect(FACTION_WAR).toBe("bong:faction/war");
  });

  it("ignores messages on unrelated channels", async () => {
    const { sub, pub } = await makeConnectedRuntime();
    sub.emit("bong:some/other/channel", JSON.stringify(makeSettlingEvent()));
    await settleMicrotasks();
    expect(pub.published).toHaveLength(0);
  });
});
