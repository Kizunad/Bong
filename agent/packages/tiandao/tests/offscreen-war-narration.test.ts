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
    expect(validateNpcDeathV1Contract(combatDeath()).ok).toBe(true);
  });

  it("agingDeath fixture is a valid NpcDeathV1 wire payload", () => {
    expect(validateNpcDeathV1Contract(agingDeath()).ok).toBe(true);
  });
});

describe("isOffscreenCombatDeath", () => {
  it("accepts only from_dormant_combat=true && cause=combat", () => {
    expect(isOffscreenCombatDeath(combatDeath())).toBe(true);
  });

  it("rejects natural_aging even if from_dormant_combat erroneously true", () => {
    expect(isOffscreenCombatDeath(agingDeath({ from_dormant_combat: true }))).toBe(false);
  });

  it("rejects combat death NOT from dormant (hydrated combat, from_dormant_combat=false)", () => {
    expect(isOffscreenCombatDeath(combatDeath({ from_dormant_combat: false }))).toBe(false);
  });

  it("rejects combat death with from_dormant_combat absent (legacy payload)", () => {
    const legacy = combatDeath();
    delete (legacy as { from_dormant_combat?: boolean }).from_dormant_combat;
    expect(isOffscreenCombatDeath(legacy)).toBe(false);
  });

  it("rejects despawned / duo_she causes", () => {
    expect(isOffscreenCombatDeath(combatDeath({ cause: "despawned" }))).toBe(false);
    expect(isOffscreenCombatDeath(combatDeath({ cause: "duo_she" }))).toBe(false);
  });
});

describe("offscreenWarRegionKey — pos quantization (NpcDeathV1 has no zone field)", () => {
  it("quantizes by X/Z cell, ignoring Y height", () => {
    const a = offscreenWarRegionKey([10, 64, 20]);
    const b = offscreenWarRegionKey([10, 200, 20]); // same X/Z, different Y
    expect(a).toBe(b);
    expect(a).toBe("region:0,0");
  });

  it("distinct cells produce distinct keys", () => {
    expect(offscreenWarRegionKey([10, 64, 20])).not.toBe(
      offscreenWarRegionKey([10 + CELL, 64, 20]),
    );
  });

  it("negative coordinates floor toward -inf (consistent cell boundaries)", () => {
    // -1 / CELL floors to cell -1, not 0
    expect(offscreenWarRegionKey([-1, 64, -1])).toBe("region:-1,-1");
    expect(offscreenWarRegionKey([0, 64, 0])).toBe("region:0,0");
  });

  it("missing pos falls back to the stable unknown region bucket", () => {
    expect(offscreenWarRegionKey(undefined)).toBe(UNKNOWN_REGION_KEY);
  });
});

describe("offscreenWarGroupKey", () => {
  it("keys by region + faction, distinct factions in same region differ", () => {
    expect(offscreenWarGroupKey("region:0,0", "attack")).not.toBe(
      offscreenWarGroupKey("region:0,0", "defend"),
    );
  });

  it("missing faction falls back to a stable 'unknown' bucket", () => {
    expect(offscreenWarGroupKey("region:0,0", undefined)).toBe("region:0,0|unknown");
  });

  it("same region+faction produces an identical, stable key", () => {
    expect(offscreenWarGroupKey("region:0,0", "attack")).toBe(
      offscreenWarGroupKey("region:0,0", "attack"),
    );
  });
});

describe("aggregateOffscreenWarReport", () => {
  it("empty sequence → zero combat deaths, no tallies", () => {
    const report = aggregateOffscreenWarReport([]);
    expect(report.totalCombatDeaths).toBe(0);
    expect(report.regions).toEqual([]);
    expect(report.factionTallies).toEqual([]);
  });

  it("single combat death → one tally, one region", () => {
    const report = aggregateOffscreenWarReport([
      combatDeath({ pos: posInCell(0, 0), faction_id: "attack" }),
    ]);
    expect(report.totalCombatDeaths).toBe(1);
    expect(report.regions).toEqual(["region:0,0"]);
    expect(report.factionTallies).toEqual([
      { regionKey: "region:0,0", factionId: "attack", deaths: 1 },
    ]);
  });

  it("aggregates_dormant_combat_deaths_by_faction — multi faction × multi region group_by(region,faction_id)", () => {
    const report = aggregateOffscreenWarReport([
      combatDeath({ npc_id: "dormant:combat:1", pos: posInCell(0, 0), faction_id: "attack" }),
      combatDeath({ npc_id: "dormant:combat:2", pos: posInCell(0, 0), faction_id: "attack" }),
      combatDeath({ npc_id: "dormant:combat:3", pos: posInCell(0, 0), faction_id: "defend" }),
      combatDeath({ npc_id: "dormant:combat:4", pos: posInCell(1, 0), faction_id: "defend" }),
    ]);
    expect(report.totalCombatDeaths).toBe(4);
    // regions 按首见顺序去重
    expect(report.regions).toEqual(["region:0,0", "region:1,0"]);
    // 三个分组：region(0,0)/attack=2, region(0,0)/defend=1, region(1,0)/defend=1
    expect(report.factionTallies).toEqual([
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
    expect(report.totalCombatDeaths).toBe(2);
    expect(report.regions).toEqual(["region:0,0"]);
    expect(report.factionTallies).toEqual([
      { regionKey: "region:0,0", factionId: "attack", deaths: 1 },
      { regionKey: "region:0,0", factionId: "defend", deaths: 1 },
    ]);
  });

  it("combat death with pos=None lands in the unknown region bucket", () => {
    const noPos = combatDeath({ npc_id: "dormant:combat:nopos", faction_id: "attack" });
    delete (noPos as { pos?: [number, number, number] }).pos;
    const report = aggregateOffscreenWarReport([noPos]);
    expect(report.totalCombatDeaths).toBe(1);
    expect(report.regions).toEqual([UNKNOWN_REGION_KEY]);
    expect(report.factionTallies).toEqual([
      { regionKey: UNKNOWN_REGION_KEY, factionId: "attack", deaths: 1 },
    ]);
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
    expect(forward.factionTallies).toEqual(reverse.factionTallies);
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
    expect(sub.subscribedChannels).toEqual([NPC_DEATH]);
  });

  it("narration_emitted_only_when_combat_deaths_present — combat death → broadcast emit", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);

    rt.handleDeathPayload(JSON.stringify(combatDeath()));
    await settleMicrotasks();

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0]?.channel).toBe(AGENT_NARRATE);
    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    expect(validateNarrationV1Contract(payload).ok).toBe(true);
    expect(payload.narrations[0]).toMatchObject({
      scope: "broadcast",
      style: "perception",
      kind: "scattered_cultivator",
    });
    expect(rt.stats.combatDeaths).toBe(1);
    expect(rt.stats.published).toBe(1);
  });

  it("narration_emitted_only_when_combat_deaths_present — only aging deaths → NO emit", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);

    rt.handleDeathPayload(JSON.stringify(agingDeath()));
    rt.handleDeathPayload(JSON.stringify(agingDeath({ npc_id: "npc:elder:2" })));
    await rt.flush();

    expect(pub.published).toHaveLength(0);
    expect(rt.stats.combatDeaths).toBe(0);
    expect(rt.stats.ignoredNonCombat).toBe(2);
    expect(rt.stats.published).toBe(0);
    expect(rt.stats.emptyFlush).toBeGreaterThanOrEqual(1);
  });

  it("empty flush (no deaths at all) does not emit", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    const result = await rt.flush();
    expect(result).toBeNull();
    expect(pub.published).toHaveLength(0);
    expect(rt.stats.emptyFlush).toBe(1);
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
    expect(narration).not.toBeNull();
    expect(pub.published).toHaveLength(1);
    // 战报只覆盖 combat 死亡所在 region（北西方），不含 aging 所在 region
    expect(narration?.text).toContain("北西方");
    expect(rt.stats.combatDeaths).toBe(2);
    expect(rt.stats.ignoredNonCombat).toBe(1);
    expect(rt.stats.published).toBe(1);
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
    expect(timers).toHaveLength(1);
    expect(pub.published).toHaveLength(0);

    timers[0]?.();
    await Promise.resolve();

    expect(pub.published).toHaveLength(1);
    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    // 两个 region 的方位都进了同一条聚合战报
    expect(/北西方/.test(payload.narrations[0].text)).toBe(true);
    expect(/南东方/.test(payload.narrations[0].text)).toBe(true);
    expect(rt.stats.published).toBe(1);
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
    expect(pub.published).toHaveLength(0); // batch 不自动 flush

    await rt.flush();
    expect(pub.published).toHaveLength(1);
    expect(rt.stats.combatDeaths).toBe(2);
    expect(rt.stats.ignoredNonCombat).toBe(1);
  });

  it("rejects non-JSON payload (contract guard)", () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    rt.handleDeathPayload("{not json");
    expect(pub.published).toHaveLength(0);
    expect(rt.stats.rejectedContract).toBe(1);
  });

  it("rejects payload failing NpcDeathV1 contract (e.g. bad cause)", () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    rt.handleDeathPayload(JSON.stringify({ ...combatDeath(), cause: "exploded" }));
    expect(pub.published).toHaveLength(0);
    expect(rt.stats.rejectedContract).toBe(1);
  });

  it("rejects payload with illegal extra field (additionalProperties:false, e.g. stray zone)", () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    // NpcDeathV1 没有 zone 字段且 additionalProperties:false —— 带 zone 的 payload 必须被拒
    rt.handleDeathPayload(JSON.stringify({ ...combatDeath(), zone: "blood_valley" }));
    expect(pub.published).toHaveLength(0);
    expect(rt.stats.rejectedContract).toBe(1);
  });

  it("routes via subscriber message dispatch (end-to-end through on('message'))", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    await rt.connect();

    sub.emit(NPC_DEATH, JSON.stringify(combatDeath()));

    expect(pub.published).toHaveLength(1);
    expect(JSON.parse(pub.published[0]?.message ?? "{}").narrations[0].kind).toBe(
      "scattered_cultivator",
    );
  });

  it("ignores messages on other channels", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = immediateRuntime(pub, sub);
    await rt.connect();

    sub.emit(CHANNELS.NPC_SPAWN, JSON.stringify(combatDeath()));

    expect(pub.published).toHaveLength(0);
    expect(rt.stats.received).toBe(0);
  });
});
