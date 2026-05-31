import { describe, expect, it, vi } from "vitest";
import {
  CHANNELS,
  type NpcDeathV1,
  validateNarrationV1Contract,
  validateNpcDeathV1Contract,
} from "@bong/schema";

import {
  OFFSCREEN_WAR_REGION_CELL_SIZE,
  OffscreenWarNarrationRuntime,
  type OffscreenWarNarrationRuntimeClient,
  UNKNOWN_REGION_KEY,
  aggregateOffscreenWarReport,
  describeRegion,
  isOffscreenCombatDeath,
  offscreenWarGroupKey,
  offscreenWarRegionKey,
  renderOffscreenWarNarration,
} from "../src/offscreen-war-narration.js";

const { AGENT_NARRATE, NPC_DEATH } = CHANNELS;

class FakePubSub implements OffscreenWarNarrationRuntimeClient {
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

/** 让 flushWindowMs=0 路径下 fire-and-forget 的 flush()（含内部 await publish）跑完。 */
function settleMicrotasks(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

const CELL = OFFSCREEN_WAR_REGION_CELL_SIZE;

/**
 * combat 战死 fixture。`NpcDeathV1` 线上无 `zone` 字段——区域只能由 `pos` 派生，故 fixture
 * 默认带 `pos`（落在原点附近 cell）。要把死亡放进不同 region，传 `pos` override。
 */
function combatDeath(overrides: Partial<NpcDeathV1> = {}): NpcDeathV1 {
  return {
    v: 1,
    kind: "npc_death",
    npc_id: "dormant:combat:a",
    archetype: "rogue",
    cause: "combat",
    faction_id: "attack",
    age_ticks: 10_000,
    max_age_ticks: 200_000,
    at_tick: 84_000,
    from_dormant_combat: true,
    pos: [12, 64, -30],
    ...overrides,
  };
}

function agingDeath(overrides: Partial<NpcDeathV1> = {}): NpcDeathV1 {
  return {
    v: 1,
    kind: "npc_death",
    npc_id: "npc:elder:1",
    archetype: "commoner",
    cause: "natural_aging",
    age_ticks: 200_000,
    max_age_ticks: 200_000,
    at_tick: 84_500,
    from_dormant_combat: false,
    ...overrides,
  };
}

/** 落在某个 region cell 中心的 pos（cx,cz 为 cell 索引）。 */
function posInCell(cx: number, cz: number): [number, number, number] {
  return [cx * CELL + CELL / 2, 64, cz * CELL + CELL / 2];
}

/** 不调度真实 timer 的 runtime（flushWindowMs=0 → 来一条 combat 死亡即 flush）。 */
function immediateRuntime(pub: FakePubSub, sub: FakePubSub): OffscreenWarNarrationRuntime {
  return new OffscreenWarNarrationRuntime({ sub, pub, logger: silent, flushWindowMs: 0 });
}

describe("fixtures pass NpcDeathV1 contract (no illegal fields)", () => {
  it("combatDeath fixture is a valid NpcDeathV1 wire payload", () => {
    const result = validateNpcDeathV1Contract(combatDeath());
    expect(
      result.ok,
      `combatDeath fixture must satisfy NpcDeathV1 (else runtime ingestion tests rest on an illegal payload), errors: ${result.errors.join("; ")}`,
    ).toBe(true);
  });

  it("agingDeath fixture is a valid NpcDeathV1 wire payload", () => {
    const result = validateNpcDeathV1Contract(agingDeath());
    expect(
      result.ok,
      `agingDeath fixture must satisfy NpcDeathV1 (the non-combat filter case must use a legal payload), errors: ${result.errors.join("; ")}`,
    ).toBe(true);
  });
});

describe("isOffscreenCombatDeath", () => {
  it("accepts only from_dormant_combat=true && cause=combat", () => {
    expect(
      isOffscreenCombatDeath(combatDeath()),
      "expected isOffscreenCombatDeath=true because from_dormant_combat===true AND cause==='combat' (both gates satisfied), actual false",
    ).toBe(true);
  });

  it("rejects natural_aging even if from_dormant_combat erroneously true", () => {
    expect(
      isOffscreenCombatDeath(agingDeath({ from_dormant_combat: true })),
      "expected isOffscreenCombatDeath=false because cause==='natural_aging' fails the cause gate even when from_dormant_combat is (wrongly) true, actual true",
    ).toBe(false);
  });

  it("rejects combat death NOT from dormant (hydrated combat, from_dormant_combat=false)", () => {
    expect(
      isOffscreenCombatDeath(combatDeath({ from_dormant_combat: false })),
      "expected isOffscreenCombatDeath=false because from_dormant_combat===false (hydrated/on-screen combat is out of scope) despite cause==='combat', actual true",
    ).toBe(false);
  });

  it("rejects combat death with from_dormant_combat absent (legacy payload)", () => {
    const legacy = combatDeath();
    delete (legacy as { from_dormant_combat?: boolean }).from_dormant_combat;
    expect(
      isOffscreenCombatDeath(legacy),
      "expected isOffscreenCombatDeath=false because from_dormant_combat absent fails the strict ===true gate (legacy payloads must not count), actual true",
    ).toBe(false);
  });

  it("rejects despawned / duo_she causes", () => {
    expect(
      isOffscreenCombatDeath(combatDeath({ cause: "despawned" })),
      "expected isOffscreenCombatDeath=false because cause==='despawned' is not 'combat', actual true",
    ).toBe(false);
    expect(
      isOffscreenCombatDeath(combatDeath({ cause: "duo_she" })),
      "expected isOffscreenCombatDeath=false because cause==='duo_she' is not 'combat', actual true",
    ).toBe(false);
  });
});

describe("offscreenWarRegionKey — pos quantization (NpcDeathV1 has no zone field)", () => {
  it("quantizes by X/Z cell, ignoring Y height", () => {
    const a = offscreenWarRegionKey([10, 64, 20]);
    const b = offscreenWarRegionKey([10, 200, 20]); // same X/Z, different Y
    expect(
      a,
      `same X/Z must map to the same region regardless of Y (war perception is planar), expected ${b}, actual ${a}`,
    ).toBe(b);
    expect(
      a,
      `[10,*,20] floors to cell (0,0) at CELL=${CELL}, expected "region:0,0", actual ${a}`,
    ).toBe("region:0,0");
  });

  it("distinct cells produce distinct keys", () => {
    const here = offscreenWarRegionKey([10, 64, 20]);
    const oneCellEast = offscreenWarRegionKey([10 + CELL, 64, 20]);
    expect(
      here,
      `points one full CELL (${CELL}) apart on X must land in different region cells, both got ${here}`,
    ).not.toBe(oneCellEast);
  });

  it("negative coordinates floor toward -inf (consistent cell boundaries)", () => {
    // -1 / CELL floors to cell -1, not 0
    expect(
      offscreenWarRegionKey([-1, 64, -1]),
      `Math.floor(-1/${CELL}) === -1 (floor toward -inf, not truncate-to-0), expected "region:-1,-1", actual ${offscreenWarRegionKey([-1, 64, -1])}`,
    ).toBe("region:-1,-1");
    expect(
      offscreenWarRegionKey([0, 64, 0]),
      `origin floors to cell (0,0), expected "region:0,0", actual ${offscreenWarRegionKey([0, 64, 0])}`,
    ).toBe("region:0,0");
  });

  it("missing pos falls back to the stable unknown region bucket", () => {
    expect(
      offscreenWarRegionKey(undefined),
      `pos=undefined must bucket into the stable unknown placeholder (NpcDeathV1 may omit pos), expected ${UNKNOWN_REGION_KEY}, actual ${offscreenWarRegionKey(undefined)}`,
    ).toBe(UNKNOWN_REGION_KEY);
  });
});

describe("offscreenWarGroupKey", () => {
  it("keys by region + faction, distinct factions in same region differ", () => {
    const attack = offscreenWarGroupKey("region:0,0", "attack");
    const defend = offscreenWarGroupKey("region:0,0", "defend");
    expect(
      attack,
      `same region but different faction must produce distinct group keys (so attack/defend tally separately), both got ${attack}`,
    ).not.toBe(defend);
  });

  it("missing faction falls back to a stable 'unknown' bucket", () => {
    expect(
      offscreenWarGroupKey("region:0,0", undefined),
      `factionId=undefined must fold into the stable 'unknown' suffix, expected "region:0,0|unknown", actual ${offscreenWarGroupKey("region:0,0", undefined)}`,
    ).toBe("region:0,0|unknown");
  });

  it("same region+faction produces an identical, stable key", () => {
    const first = offscreenWarGroupKey("region:0,0", "attack");
    const second = offscreenWarGroupKey("region:0,0", "attack");
    expect(
      first,
      `identical (region,faction) inputs must yield a byte-identical key (group_by depends on it), expected ${second}, actual ${first}`,
    ).toBe(second);
  });
});

describe("aggregateOffscreenWarReport", () => {
  it("empty sequence → zero combat deaths, no tallies", () => {
    const report = aggregateOffscreenWarReport([]);
    expect(
      report.totalCombatDeaths,
      `empty input must aggregate to 0 combat deaths, actual ${report.totalCombatDeaths}`,
    ).toBe(0);
    expect(
      report.regions,
      `empty input must yield no regions, actual ${JSON.stringify(report.regions)}`,
    ).toEqual([]);
    expect(
      report.factionTallies,
      `empty input must yield no faction tallies, actual ${JSON.stringify(report.factionTallies)}`,
    ).toEqual([]);
  });

  it("single combat death → one tally, one region", () => {
    const report = aggregateOffscreenWarReport([
      combatDeath({ pos: posInCell(0, 0), faction_id: "attack" }),
    ]);
    expect(
      report.totalCombatDeaths,
      `one combat death must count as 1, actual ${report.totalCombatDeaths}`,
    ).toBe(1);
    expect(
      report.regions,
      `pos at cell (0,0) must map to exactly ["region:0,0"], actual ${JSON.stringify(report.regions)}`,
    ).toEqual(["region:0,0"]);
    expect(
      report.factionTallies,
      `single attack death at (0,0) must produce one tally {region:0,0, attack, deaths:1}, actual ${JSON.stringify(report.factionTallies)}`,
    ).toEqual([{ regionKey: "region:0,0", factionId: "attack", deaths: 1 }]);
  });

  it("aggregates_dormant_combat_deaths_by_faction — multi faction × multi region group_by(region,faction_id)", () => {
    const report = aggregateOffscreenWarReport([
      combatDeath({ npc_id: "dormant:combat:1", pos: posInCell(0, 0), faction_id: "attack" }),
      combatDeath({ npc_id: "dormant:combat:2", pos: posInCell(0, 0), faction_id: "attack" }),
      combatDeath({ npc_id: "dormant:combat:3", pos: posInCell(0, 0), faction_id: "defend" }),
      combatDeath({ npc_id: "dormant:combat:4", pos: posInCell(1, 0), faction_id: "defend" }),
    ]);
    expect(
      report.totalCombatDeaths,
      `all 4 inputs are combat deaths, expected totalCombatDeaths=4, actual ${report.totalCombatDeaths}`,
    ).toBe(4);
    // regions 按首见顺序去重
    expect(
      report.regions,
      `two distinct cells touched, deduped in first-seen order, expected ["region:0,0","region:1,0"], actual ${JSON.stringify(report.regions)}`,
    ).toEqual(["region:0,0", "region:1,0"]);
    // 三个分组：region(0,0)/attack=2, region(0,0)/defend=1, region(1,0)/defend=1
    expect(
      report.factionTallies,
      `group_by(region,faction) must split into 3 buckets [(0,0)/attack=2, (0,0)/defend=1, (1,0)/defend=1] sorted by group key, actual ${JSON.stringify(report.factionTallies)}`,
    ).toEqual([
      { regionKey: "region:0,0", factionId: "attack", deaths: 2 },
      { regionKey: "region:0,0", factionId: "defend", deaths: 1 },
      { regionKey: "region:1,0", factionId: "defend", deaths: 1 },
    ]);
  });

  it("ignores_natural_aging_in_war_narration — mixed combat+aging, only combat enters report", () => {
    const report = aggregateOffscreenWarReport([
      combatDeath({ npc_id: "dormant:combat:1", pos: posInCell(0, 0), faction_id: "attack" }),
      agingDeath({ npc_id: "npc:elder:1", pos: posInCell(5, 5) }),
      // 被错标 from_dormant_combat=true 的 aging 也必须被 cause 门槛挡掉
      agingDeath({ npc_id: "npc:elder:2", pos: posInCell(0, 0), from_dormant_combat: true }),
      combatDeath({ npc_id: "dormant:combat:2", pos: posInCell(0, 0), faction_id: "defend" }),
    ]);
    // aging（含被错标 from_dormant_combat=true 的那条）全部不计；region(5,5) 不出现
    expect(
      report.totalCombatDeaths,
      `only the 2 combat deaths count; both aging entries (incl. the mislabeled from_dormant_combat=true one) are filtered, expected 2, actual ${report.totalCombatDeaths}`,
    ).toBe(2);
    expect(
      report.regions,
      `aging at cell (5,5) must not leak into regions; only combat cell (0,0) remains, expected ["region:0,0"], actual ${JSON.stringify(report.regions)}`,
    ).toEqual(["region:0,0"]);
    expect(
      report.factionTallies,
      `only combat deaths tally: (0,0)/attack=1, (0,0)/defend=1, actual ${JSON.stringify(report.factionTallies)}`,
    ).toEqual([
      { regionKey: "region:0,0", factionId: "attack", deaths: 1 },
      { regionKey: "region:0,0", factionId: "defend", deaths: 1 },
    ]);
  });

  it("combat death with pos=None lands in the unknown region bucket", () => {
    const noPos = combatDeath({ npc_id: "dormant:combat:nopos", faction_id: "attack" });
    delete (noPos as { pos?: [number, number, number] }).pos;
    const report = aggregateOffscreenWarReport([noPos]);
    expect(
      report.totalCombatDeaths,
      `a pos-less combat death still counts as 1 combat death, actual ${report.totalCombatDeaths}`,
    ).toBe(1);
    expect(
      report.regions,
      `pos=None must bucket into the unknown region, expected [${UNKNOWN_REGION_KEY}], actual ${JSON.stringify(report.regions)}`,
    ).toEqual([UNKNOWN_REGION_KEY]);
    expect(
      report.factionTallies,
      `pos-less attack death tallies under (${UNKNOWN_REGION_KEY}, attack, 1), actual ${JSON.stringify(report.factionTallies)}`,
    ).toEqual([{ regionKey: UNKNOWN_REGION_KEY, factionId: "attack", deaths: 1 }]);
  });

  it("aggregation order is deterministic regardless of input order (sorted by group key)", () => {
    const forward = aggregateOffscreenWarReport([
      combatDeath({ npc_id: "x1", pos: posInCell(1, 0), faction_id: "defend" }),
      combatDeath({ npc_id: "x2", pos: posInCell(0, 0), faction_id: "attack" }),
    ]);
    const reverse = aggregateOffscreenWarReport([
      combatDeath({ npc_id: "x2", pos: posInCell(0, 0), faction_id: "attack" }),
      combatDeath({ npc_id: "x1", pos: posInCell(1, 0), faction_id: "defend" }),
    ]);
    expect(
      forward.factionTallies,
      `tallies must be sorted by group key, so input order must not change the result, forward=${JSON.stringify(forward.factionTallies)} reverse=${JSON.stringify(reverse.factionTallies)}`,
    ).toEqual(reverse.factionTallies);
  });
});

describe("describeRegion — anonymous directional framing (no concrete place names)", () => {
  it("unknown region → 不知名的荒野", () => {
    expect(describeRegion(UNKNOWN_REGION_KEY)).toBe("不知名的荒野");
  });

  it("origin cell → 近处 (no direction)", () => {
    expect(describeRegion("region:0,0")).toBe("近处");
  });

  it("negative Z (north) / positive X (east) compose to 北东方", () => {
    expect(describeRegion("region:1,-1")).toBe("北东方");
  });

  it("positive Z (south) / negative X (west) compose to 南西方", () => {
    expect(describeRegion("region:-1,1")).toBe("南西方");
  });

  it("malformed key → 不知名的荒野 (defensive)", () => {
    expect(describeRegion("garbage")).toBe("不知名的荒野");
  });
});

describe("renderOffscreenWarNarration — anonymous scattered-cultivator framing (worldview §七/§十)", () => {
  // P4 是脊柱层：只能匿名散修。具名宗门是 P5（gated，缺 worldview 锚点）。
  const NAMED_SECTS = ["玄岭", "断魂", "青云", "沧渊", "宗门"];

  it("broadcast scope + perception style + scattered_cultivator kind", () => {
    const narration = renderOffscreenWarNarration(aggregateOffscreenWarReport([combatDeath()]));
    expect(narration.scope).toBe("broadcast");
    expect(narration.style).toBe("perception");
    expect(narration.kind).toBe("scattered_cultivator");
    expect(narration.target).toBeUndefined();
  });

  it("never names a concrete sect across all 3 deterministic template variants", () => {
    for (let deaths = 1; deaths <= 3; deaths += 1) {
      const events = Array.from({ length: deaths }, (_, i) =>
        combatDeath({ npc_id: `dormant:combat:${i}`, pos: posInCell(-1, -1) }),
      );
      const narration = renderOffscreenWarNarration(aggregateOffscreenWarReport(events));
      for (const banned of NAMED_SECTS) {
        expect(
          narration.text.includes(banned),
          `narration must stay anonymous (no '${banned}' named sect), got: ${narration.text}`,
        ).toBe(false);
      }
      // 仍含散修群体涌现语义锚点之一
      expect(/散修|无名|派系|修士/.test(narration.text)).toBe(true);
    }
  });

  it("embeds an anonymous directional label (not a precise place name) into the text", () => {
    const narration = renderOffscreenWarNarration(
      aggregateOffscreenWarReport([
        combatDeath({ npc_id: "a", pos: posInCell(-1, -1) }), // 北西方
        combatDeath({ npc_id: "b", pos: posInCell(-1, -1) }),
        combatDeath({ npc_id: "c", pos: posInCell(1, 1) }), // 南东方
      ]),
    );
    // 方位锚点出现，但绝不出现 region cell 索引或 zone 名等"测绘级"信息
    expect(/北西方|南东方/.test(narration.text)).toBe(true);
    expect(narration.text).not.toContain("region:");
    expect(narration.text).not.toMatch(/blood_valley|north_wastes/);
  });

  it("passes NarrationV1 contract for every variant", () => {
    for (let deaths = 1; deaths <= 6; deaths += 1) {
      const events = Array.from({ length: deaths }, (_, i) =>
        combatDeath({ npc_id: `dormant:combat:${i}` }),
      );
      const narration = renderOffscreenWarNarration(aggregateOffscreenWarReport(events));
      expect(validateNarrationV1Contract({ v: 1, narrations: [narration] }).ok).toBe(true);
    }
  });
});

describe("OffscreenWarNarrationRuntime", () => {
  it("subscribes only to bong:npc/death (reuses existing channel, no new telemetry channel)", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    await rt.connect();
    expect(
      sub.subscribedChannels,
      `runtime must reuse the existing ${NPC_DEATH} channel only (no new telemetry channel), expected [${NPC_DEATH}], actual ${JSON.stringify(sub.subscribedChannels)}`,
    ).toEqual([NPC_DEATH]);
  });

  it("narration_emitted_only_when_combat_deaths_present — combat death → broadcast emit", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);

    rt.handleDeathPayload(JSON.stringify(combatDeath()));
    await settleMicrotasks();

    expect(
      pub.published.length,
      `one combat death (flushWindowMs=0) must emit exactly one narration, actual ${pub.published.length}`,
    ).toBe(1);
    expect(
      pub.published[0]?.channel,
      `narration must publish on ${AGENT_NARRATE} (reused agent channel), actual ${pub.published[0]?.channel}`,
    ).toBe(AGENT_NARRATE);
    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    const narrationValidation = validateNarrationV1Contract(payload);
    expect(
      narrationValidation.ok,
      `emitted payload must satisfy NarrationV1 (server consumes it), errors: ${narrationValidation.errors.join("; ")}`,
    ).toBe(true);
    expect(payload.narrations[0]).toMatchObject({
      scope: "broadcast",
      style: "perception",
      kind: "scattered_cultivator",
    });
    expect(
      rt.stats.combatDeaths,
      `one combat death must increment combatDeaths to 1, actual ${rt.stats.combatDeaths}`,
    ).toBe(1);
    expect(
      rt.stats.published,
      `a successful emit must increment published to 1, actual ${rt.stats.published}`,
    ).toBe(1);
  });

  it("narration_emitted_only_when_combat_deaths_present — only aging deaths → NO emit", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);

    rt.handleDeathPayload(JSON.stringify(agingDeath()));
    rt.handleDeathPayload(JSON.stringify(agingDeath({ npc_id: "npc:elder:2" })));
    await rt.flush();

    expect(
      pub.published.length,
      `aging-only window has no combat deaths, so nothing may be emitted, actual ${pub.published.length} emits`,
    ).toBe(0);
    expect(
      rt.stats.combatDeaths,
      `no combat deaths in an aging-only window, expected combatDeaths=0, actual ${rt.stats.combatDeaths}`,
    ).toBe(0);
    expect(
      rt.stats.ignoredNonCombat,
      `both aging deaths must count as ignoredNonCombat, expected 2, actual ${rt.stats.ignoredNonCombat}`,
    ).toBe(2);
    expect(
      rt.stats.published,
      `aging-only window must not publish, expected published=0, actual ${rt.stats.published}`,
    ).toBe(0);
    expect(
      rt.stats.emptyFlush,
      `a flush over an empty (combat-less) buffer must increment emptyFlush at least once, actual ${rt.stats.emptyFlush}`,
    ).toBeGreaterThanOrEqual(1);
  });

  it("empty flush (no deaths at all) does not emit", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    const result = await rt.flush();
    expect(
      result,
      `flush over a never-fed buffer must return null (nothing to aggregate), actual ${JSON.stringify(result)}`,
    ).toBeNull();
    expect(
      pub.published.length,
      `empty flush must not publish, actual ${pub.published.length} emits`,
    ).toBe(0);
    expect(
      rt.stats.emptyFlush,
      `one flush over an empty buffer must set emptyFlush to exactly 1, actual ${rt.stats.emptyFlush}`,
    ).toBe(1);
  });

  it("mixed window (combat + aging) → emits exactly one report covering only combat deaths", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    // 窗口模式：buffer 攒齐后手动 flush，验证一窗一条聚合战报
    const rt = new OffscreenWarNarrationRuntime({ sub, pub, logger: silent, flushWindowMs: 60_000 });

    rt.handleNpcDeath(combatDeath({ npc_id: "dormant:combat:1", pos: posInCell(-1, -1), faction_id: "attack" }));
    rt.handleNpcDeath(agingDeath({ npc_id: "npc:elder:1", pos: posInCell(9, 9) }));
    rt.handleNpcDeath(combatDeath({ npc_id: "dormant:combat:2", pos: posInCell(-1, -1), faction_id: "defend" }));

    const narration = await rt.flush();
    expect(
      narration,
      "a window containing combat deaths must produce a non-null narration, actual null",
    ).not.toBeNull();
    expect(
      pub.published.length,
      `one window flush must emit exactly one aggregated report, actual ${pub.published.length}`,
    ).toBe(1);
    // 战报只覆盖 combat 死亡所在 region（北西方），不含 aging 所在 region
    expect(
      narration?.text,
      `narration must reference the combat region (北西方) only; aging at (9,9) must not appear, actual: ${narration?.text}`,
    ).toContain("北西方");
    expect(
      rt.stats.combatDeaths,
      `2 of the 3 deaths are combat, expected combatDeaths=2, actual ${rt.stats.combatDeaths}`,
    ).toBe(2);
    expect(
      rt.stats.ignoredNonCombat,
      `the lone aging death must count as ignoredNonCombat=1, actual ${rt.stats.ignoredNonCombat}`,
    ).toBe(1);
    expect(
      rt.stats.published,
      `the single window flush must increment published to 1, actual ${rt.stats.published}`,
    ).toBe(1);
  });

  it("window debounce: multiple combat deaths within one window → single aggregated emit", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const timers: Array<() => void> = [];
    const rt = new OffscreenWarNarrationRuntime({
      sub,
      pub,
      logger: silent,
      flushWindowMs: 3000,
      // 同步桩 timer：记录 cb，由测试在攒满后手动触发
      setTimer: (cb: () => void) => {
        timers.push(cb);
        return 0 as unknown as ReturnType<typeof setTimeout>;
      },
      clearTimer: () => {},
    });

    rt.handleNpcDeath(combatDeath({ npc_id: "dormant:combat:1", pos: posInCell(-1, -1), faction_id: "attack" }));
    rt.handleNpcDeath(combatDeath({ npc_id: "dormant:combat:2", pos: posInCell(-1, -1), faction_id: "defend" }));
    rt.handleNpcDeath(combatDeath({ npc_id: "dormant:combat:3", pos: posInCell(1, 1), faction_id: "attack" }));

    // 一窗内 3 笔战死只调度一次 flush timer（debounce）
    expect(
      timers.length,
      `debounce: 3 combat deaths inside one open window must arm only one flush timer, actual ${timers.length}`,
    ).toBe(1);
    expect(
      pub.published.length,
      `nothing may be emitted until the window timer fires, actual ${pub.published.length} pre-flush emits`,
    ).toBe(0);

    timers[0]?.();
    await Promise.resolve();

    expect(
      pub.published.length,
      `firing the single window timer must flush exactly one aggregated report, actual ${pub.published.length}`,
    ).toBe(1);
    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    // 两个 region 的方位都进了同一条聚合战报
    expect(
      /北西方/.test(payload.narrations[0].text),
      `both regions must fold into one report; missing 北西方 (cell -1,-1) in: ${payload.narrations[0].text}`,
    ).toBe(true);
    expect(
      /南东方/.test(payload.narrations[0].text),
      `both regions must fold into one report; missing 南东方 (cell 1,1) in: ${payload.narrations[0].text}`,
    ).toBe(true);
    expect(
      rt.stats.published,
      `a single debounced flush must increment published to 1, actual ${rt.stats.published}`,
    ).toBe(1);
  });

  it("handleNpcDeathBatch buffers without scheduling timers; flush emits once", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new OffscreenWarNarrationRuntime({ sub, pub, logger: silent, flushWindowMs: 60_000 });

    rt.handleNpcDeathBatch([
      combatDeath({ npc_id: "dormant:combat:1", pos: posInCell(-1, -1), faction_id: "attack" }),
      agingDeath({ npc_id: "npc:elder:1" }),
      combatDeath({ npc_id: "dormant:combat:2", pos: posInCell(1, 1), faction_id: "defend" }),
    ]);
    expect(
      pub.published.length,
      `handleNpcDeathBatch must buffer only (no auto-flush), expected 0 emits before flush(), actual ${pub.published.length}`,
    ).toBe(0); // batch 不自动 flush

    await rt.flush();
    expect(
      pub.published.length,
      `manual flush after batch must emit exactly one aggregated report, actual ${pub.published.length}`,
    ).toBe(1);
    expect(
      rt.stats.combatDeaths,
      `the batch had 2 combat deaths, expected combatDeaths=2, actual ${rt.stats.combatDeaths}`,
    ).toBe(2);
    expect(
      rt.stats.ignoredNonCombat,
      `the batch had 1 aging death, expected ignoredNonCombat=1, actual ${rt.stats.ignoredNonCombat}`,
    ).toBe(1);
  });

  it("rejects non-JSON payload (contract guard)", () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    rt.handleDeathPayload("{not json");
    expect(
      pub.published.length,
      `a malformed-JSON payload must be dropped, never emitted, actual ${pub.published.length} emits`,
    ).toBe(0);
    expect(
      rt.stats.rejectedContract,
      `JSON.parse failure must increment rejectedContract to 1, actual ${rt.stats.rejectedContract}`,
    ).toBe(1);
  });

  it("rejects payload failing NpcDeathV1 contract (e.g. bad cause)", () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    rt.handleDeathPayload(JSON.stringify({ ...combatDeath(), cause: "exploded" }));
    expect(
      pub.published.length,
      `an out-of-enum cause must fail the NpcDeathV1 guard and emit nothing, actual ${pub.published.length} emits`,
    ).toBe(0);
    expect(
      rt.stats.rejectedContract,
      `a contract-invalid payload (bad cause) must increment rejectedContract to 1, actual ${rt.stats.rejectedContract}`,
    ).toBe(1);
  });

  it("rejects payload with illegal extra field (additionalProperties:false, e.g. stray zone)", () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    // NpcDeathV1 没有 zone 字段且 additionalProperties:false —— 带 zone 的 payload 必须被拒
    rt.handleDeathPayload(JSON.stringify({ ...combatDeath(), zone: "blood_valley" }));
    expect(
      pub.published.length,
      `NpcDeathV1 has additionalProperties:false, so a stray 'zone' field must be rejected (no emit), actual ${pub.published.length} emits`,
    ).toBe(0);
    expect(
      rt.stats.rejectedContract,
      `an illegal extra field must increment rejectedContract to 1, actual ${rt.stats.rejectedContract}`,
    ).toBe(1);
  });

  it("routes via subscriber message dispatch (end-to-end through on('message'))", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    await rt.connect();

    sub.emit(NPC_DEATH, JSON.stringify(combatDeath()));

    expect(
      pub.published.length,
      `a death delivered via on('message') dispatch must flow end-to-end to one emit, actual ${pub.published.length}`,
    ).toBe(1);
    expect(
      JSON.parse(pub.published[0]?.message ?? "{}").narrations[0].kind,
      `the dispatched-through narration must keep kind='scattered_cultivator', actual ${JSON.parse(pub.published[0]?.message ?? "{}").narrations[0].kind}`,
    ).toBe("scattered_cultivator");
  });

  it("ignores messages on other channels", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    await rt.connect();

    sub.emit(CHANNELS.NPC_SPAWN, JSON.stringify(combatDeath()));

    expect(
      pub.published.length,
      `non-death channel (${CHANNELS.NPC_SPAWN}) must be filtered by onMessage's channel guard, expected 0 emits, actual ${pub.published.length}`,
    ).toBe(0);
    expect(
      rt.stats.received,
      `messages on channels other than ${NPC_DEATH} must never reach handleNpcDeath, expected received=0, actual ${rt.stats.received}`,
    ).toBe(0);
  });
});

describe("OffscreenWarNarrationRuntime — lifecycle state transitions (connect/disconnect idempotency & re-routing)", () => {
  /**
   * 同步桩 timer 的 runtime 工厂：setTimer 只记录 cb（绝不自动执行），clearTimer 记录被清的 handle。
   * 让 connect→disconnect→reconnect 全程 timer 行为可观察、可断言（不依赖真实 setTimeout 时序）。
   */
  function instrumentedRuntime(
    pub: FakePubSub,
    sub: FakePubSub,
    flushWindowMs: number,
  ): {
    rt: OffscreenWarNarrationRuntime;
    timerCallbacks: Array<() => void>;
    clearedHandles: Array<ReturnType<typeof setTimeout>>;
    timerHandles: Array<ReturnType<typeof setTimeout>>;
  } {
    const timerCallbacks: Array<() => void> = [];
    const clearedHandles: Array<ReturnType<typeof setTimeout>> = [];
    const timerHandles: Array<ReturnType<typeof setTimeout>> = [];
    let nextHandle = 1;
    const rt = new OffscreenWarNarrationRuntime({
      sub,
      pub,
      logger: silent,
      flushWindowMs,
      setTimer: (cb: () => void) => {
        timerCallbacks.push(cb);
        const handle = nextHandle++ as unknown as ReturnType<typeof setTimeout>;
        timerHandles.push(handle);
        return handle;
      },
      clearTimer: (handle: ReturnType<typeof setTimeout>) => {
        clearedHandles.push(handle);
      },
    });
    return { rt, timerCallbacks, clearedHandles, timerHandles };
  }

  // connect → connect (A→A)：幂等，已 connected 直接 return，不重复订阅 / 不重复挂监听。
  it("connect is idempotent (A→A): double connect subscribes once and registers a single listener", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);

    await rt.connect();
    await rt.connect();

    expect(
      sub.subscribedChannels,
      `connect() must early-return when already connected, so subscribe(${NPC_DEATH}) fires exactly once, actual ${JSON.stringify(sub.subscribedChannels)}`,
    ).toEqual([NPC_DEATH]);
    expect(
      sub.listeners.length,
      `repeated connect() must not stack 'message' listeners (else one death emits twice), expected 1 listener, actual ${sub.listeners.length}`,
    ).toBe(1);
  });

  // connect → connect → emit (A→A then receive)：幂等后仍只路由一次，不会因重复监听双发。
  it("idempotent connect does not double-deliver: one death after double connect emits exactly one narration", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);

    await rt.connect();
    await rt.connect();
    sub.emit(NPC_DEATH, JSON.stringify(combatDeath()));
    await settleMicrotasks();

    expect(
      pub.published.length,
      `a single combat death must yield a single emit even after double connect (no listener stacking), expected 1, actual ${pub.published.length}`,
    ).toBe(1);
    expect(
      rt.stats.received,
      `the death must be counted once, not once-per-stacked-listener, expected received=1, actual ${rt.stats.received}`,
    ).toBe(1);
  });

  // connect → disconnect (A→B)：摘监听后再喂消息，不进 buffer、不 emit。
  it("disconnect stops routing (A→B): a death message after disconnect is neither buffered nor emitted", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);

    await rt.connect();
    await rt.disconnect();

    expect(
      sub.listeners.length,
      `disconnect() must off() the 'message' listener, expected 0 listeners, actual ${sub.listeners.length}`,
    ).toBe(0);

    // 监听已摘 → emit 落空（FakePubSub.emit 遍历空 listeners）。
    sub.emit(NPC_DEATH, JSON.stringify(combatDeath()));
    await settleMicrotasks();

    expect(
      pub.published.length,
      `post-disconnect death must not reach the runtime (listener detached), expected 0 emits, actual ${pub.published.length}`,
    ).toBe(0);
    expect(
      rt.stats.received,
      `post-disconnect death must not be counted (handler unsubscribed), expected received=0, actual ${rt.stats.received}`,
    ).toBe(0);
  });

  // connect → disconnect → connect (A→B→A)：reconnect 重新订阅并恢复路由，combat death 正常聚合 emit。
  it("reconnect restores routing (A→B→A): combat death after re-connect aggregates and emits", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);

    await rt.connect();
    await rt.disconnect();
    await rt.connect();

    expect(
      sub.subscribedChannels,
      `reconnect re-subscribes (connected was reset to false by disconnect), so ${NPC_DEATH} appears twice across the two connects, actual ${JSON.stringify(sub.subscribedChannels)}`,
    ).toEqual([NPC_DEATH, NPC_DEATH]);
    expect(
      sub.listeners.length,
      `reconnect must leave exactly one live 'message' listener (old one off()ed at disconnect), expected 1, actual ${sub.listeners.length}`,
    ).toBe(1);

    sub.emit(NPC_DEATH, JSON.stringify(combatDeath({ pos: posInCell(-1, -1), faction_id: "attack" })));
    await settleMicrotasks();

    expect(
      pub.published.length,
      `routing must be live again after reconnect, expected 1 emit, actual ${pub.published.length}`,
    ).toBe(1);
    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    expect(
      payload.narrations?.[0]?.kind,
      `reconnected runtime must still produce the scattered_cultivator narration, actual ${JSON.stringify(payload.narrations?.[0]?.kind)}`,
    ).toBe("scattered_cultivator");
    expect(
      rt.stats.published,
      `reconnected combat death must increment published, expected published=1, actual ${rt.stats.published}`,
    ).toBe(1);
  });

  // disconnect 清理 pending flush timer：clearTimer 命中正确 handle、flushHandle 复位、之后不迟发。
  it("disconnect clears a pending flush timer (A→B with armed window): clearTimer fires on the live handle and no late narration leaks", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const { rt, timerCallbacks, clearedHandles, timerHandles } = instrumentedRuntime(pub, sub, 3000);

    await rt.connect();
    // combat death 武装窗口 timer（flushWindowMs>0 → scheduleFlush 设置 flushHandle）。
    rt.handleNpcDeath(combatDeath({ pos: posInCell(-1, -1), faction_id: "attack" }));

    expect(
      timerCallbacks.length,
      `first combat death must arm exactly one window flush timer, expected 1, actual ${timerCallbacks.length}`,
    ).toBe(1);
    expect(
      pub.published.length,
      `windowed runtime must not emit before the timer fires, expected 0 emits pre-flush, actual ${pub.published.length}`,
    ).toBe(0);

    await rt.disconnect();

    expect(
      clearedHandles,
      `disconnect must clearTimer() the armed flush handle (else a late flush leaks after teardown), expected the live handle ${JSON.stringify(timerHandles)}, actual ${JSON.stringify(clearedHandles)}`,
    ).toEqual(timerHandles);

    // flushHandle 复位后，再喂 combat death 不会复用旧 handle（scheduleFlush 会新设一个）——
    // 但此处更关键：teardown 后不应有任何 narration 迟发。timer cb 由同步桩持有、绝不自动触发，
    // 故 pub.published 必须仍为空，证明窗口被干净取消、无迟发。
    await settleMicrotasks();
    expect(
      pub.published.length,
      `after disconnect cancels the window, no narration may be published, expected 0 emits, actual ${pub.published.length}`,
    ).toBe(0);
  });
});
