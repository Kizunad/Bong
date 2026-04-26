import { describe, expect, it } from "vitest";
import type { PlayerProfile, WorldStateV1, ZoneSnapshot } from "@bong/schema";
import { WorldModel } from "../src/world-model.js";

interface PlayerOverrides extends Partial<Omit<PlayerProfile, "breakdown" | "pos" | "name">> {
  breakdown?: Partial<PlayerProfile["breakdown"]>;
  pos?: PlayerProfile["pos"];
}

function createPlayer(name: string, overrides: PlayerOverrides = {}): PlayerProfile {
  return {
    uuid: overrides.uuid ?? `offline:${name}`,
    name,
    realm: overrides.realm ?? "Awaken",
    composite_power: overrides.composite_power ?? 0.2,
    breakdown: {
      combat: 0.2,
      wealth: 0.2,
      social: 0.2,
      karma: 0,
      territory: 0.2,
      ...overrides.breakdown,
    },
    trend: overrides.trend ?? "stable",
    active_hours: overrides.active_hours ?? 1,
    zone: overrides.zone ?? "starter_zone",
    pos: overrides.pos ?? [0, 64, 0],
    recent_kills: overrides.recent_kills ?? 0,
    recent_deaths: overrides.recent_deaths ?? 0,
    cultivation: overrides.cultivation,
    life_record: overrides.life_record,
  };
}

function createZone(name: string, spiritQi: number, overrides: Partial<ZoneSnapshot> = {}): ZoneSnapshot {
  return {
    name,
    spirit_qi: spiritQi,
    danger_level: overrides.danger_level ?? 1,
    active_events: overrides.active_events ?? [],
    player_count: overrides.player_count ?? 0,
  };
}

function createState(args: {
  tick: number;
  players?: PlayerProfile[];
  zones?: ZoneSnapshot[];
}): WorldStateV1 {
  const players = args.players ?? [];
  const zones =
    args.zones ?? [createZone("starter_zone", 0.5, { player_count: players.length })];

  return {
    v: 1,
    ts: 1_710_000_000 + args.tick,
    tick: args.tick,
    players,
    npcs: [],
    zones,
    recent_events: [],
  };
}

describe("WorldModel", () => {
  it("keeps the last 10 zone snapshots and calculates falling trends", () => {
    const model = new WorldModel();
    const spirits = [0.64, 0.62, 0.6, 0.58, 0.56, 0.54, 0.52, 0.5, 0.48, 0.46, 0.44, 0.42];

    spirits.forEach((spiritQi, index) => {
      model.updateState(
        createState({
          tick: index + 1,
          zones: [createZone("blood_valley", spiritQi)],
        }),
      );
    });

    const history = model.getZoneHistory("blood_valley");
    const trend = model.getZoneTrendSummary("blood_valley");

    expect(history).toHaveLength(10);
    expect(history[0]?.spirit_qi).toBeCloseTo(0.6, 6);
    expect(history.at(-1)?.spirit_qi).toBeCloseTo(0.42, 6);
    expect(trend).not.toBeNull();
    expect(trend?.previousSpiritQi).toBeCloseTo(0.5, 6);
    expect(trend?.currentSpiritQi).toBeCloseTo(0.44, 6);
    expect(trend?.delta).toBeCloseTo(-0.06, 6);
    expect(trend?.trend).toBe("falling");
  });

  it("identifies key players and preserves previous peer decisions for the next round", () => {
    const model = new WorldModel();

    const recurringPlayers = [
      createPlayer("Steve", {
        composite_power: 0.98,
        zone: "blood_valley",
        recent_kills: 8,
        breakdown: { combat: 0.95, karma: -0.45 },
      }),
      createPlayer("Keeper", {
        composite_power: 0.15,
        zone: "green_cloud_peak",
        breakdown: { social: 0.5, karma: 0.2 },
      }),
      createPlayer("Wanderer", {
        composite_power: 0.05,
        zone: "newbie_valley",
        breakdown: { karma: 0 },
      }),
    ];

    model.updateState(
      createState({
        tick: 1,
        players: recurringPlayers,
        zones: [
          createZone("blood_valley", 0.6, { player_count: 1 }),
          createZone("green_cloud_peak", 0.82, { player_count: 1 }),
          createZone("newbie_valley", 0.93, { player_count: 1 }),
        ],
      }),
    );

    model.recordDecision("calamity", {
      commands: [
        {
          type: "spawn_event",
          target: "blood_valley",
          params: { event: "thunder_tribulation", intensity: 0.6 },
        },
      ],
      narrations: [],
      reasoning: "punish the strongest",
    });
    model.recordDecision("mutation", {
      commands: [
        { type: "modify_zone", target: "blood_valley", params: { spirit_qi_delta: -0.05 } },
        { type: "modify_zone", target: "green_cloud_peak", params: { spirit_qi_delta: 0.05 } },
      ],
      narrations: [],
      reasoning: "rebalance resources",
    });
    model.recordDecision("era", {
      commands: [],
      narrations: [],
      reasoning: "observe this round",
    });

    model.updateState(
      createState({
        tick: 2,
        players: [
          ...recurringPlayers,
          createPlayer("FreshFace", {
            composite_power: 0.02,
            zone: "newbie_valley",
            breakdown: { karma: 0 },
          }),
        ],
        zones: [
          createZone("blood_valley", 0.52, { player_count: 1 }),
          createZone("green_cloud_peak", 0.86, { player_count: 1 }),
          createZone("newbie_valley", 0.95, { player_count: 2 }),
        ],
      }),
    );

    const keyPlayers = model.getKeyPlayers();
    const steve = keyPlayers.find((player) => player.name === "Steve");
    const freshFace = keyPlayers.find((player) => player.name === "FreshFace");
    const peerDecisions = model.getPeerDecisions("calamity");

    expect(steve?.reasons).toEqual(
      expect.arrayContaining(["综合最强(0.98)", "karma 偏负(-0.45)", "连续击杀 8 次"]),
    );
    expect(steve?.note).toBe("因果将至");
    expect(freshFace?.reasons).toEqual(expect.arrayContaining(["新入世(0.02)"]));
    expect(freshFace?.note).toBe("天道可扶");
    expect(peerDecisions.map((decision) => decision.agentName)).toEqual(["mutation", "era"]);
    expect(peerDecisions[0]?.summary).toContain("blood_valley 灵气 -0.05");
    expect(peerDecisions[0]?.summary).toContain("green_cloud_peak 灵气 +0.05");
    expect(peerDecisions[1]?.summary).toBe("无行动");
  });

  it("surfaces recent skill milestone breakthroughs in key player reasons", () => {
    const model = new WorldModel();

    model.updateState(
      createState({
        tick: 1,
        players: [
          createPlayer("Craftsman", {
            composite_power: 0.42,
            zone: "green_cloud_peak",
            life_record: {
              recent_biography_summary: "t82000:reach:Spirit",
              recent_skill_milestones_summary: "t83000:skill:alchemy:lv4",
              skill_milestones: [
                {
                  skill: "alchemy",
                  new_lv: 4,
                  achieved_at: 83000,
                  narration: "炉火识性渐深，丹道已至Lv.4。",
                  total_xp_at: 1200,
                },
              ],
            },
          }),
        ],
        zones: [createZone("green_cloud_peak", 0.82, { player_count: 1 })],
      }),
    );

    const keyPlayers = model.getKeyPlayers();
    const craftsman = keyPlayers.find((player) => player.name === "Craftsman");

    expect(craftsman?.reasons).toContain("技艺突破 炼丹 Lv.4");
    expect(craftsman?.note).toBe("技艺突破 炼丹 Lv.4，手艺有成");
  });

  it("persists current era state with structured global effect", () => {
    const model = new WorldModel();

    model.setCurrentEra({
      name: "末法纪",
      sinceTick: 321,
      globalEffect: "灵机渐枯，诸域修行更艰",
    });

    expect(model.currentEra).toEqual({
      name: "末法纪",
      sinceTick: 321,
      globalEffect: "灵机渐枯，诸域修行更艰",
    });
  });

  it("round-trips durable snapshot via toJSON/fromJSON and keeps ephemeral newcomers out", () => {
    const model = new WorldModel();

    model.updateState(
      createState({
        tick: 99,
        players: [
          createPlayer("Elder", {
            composite_power: 0.88,
            zone: "blood_valley",
            breakdown: { karma: -0.3 },
          }),
        ],
        zones: [createZone("blood_valley", 0.66, { player_count: 1 })],
      }),
    );
    model.updateState(
      createState({
        tick: 100,
        players: [
          createPlayer("Elder", {
            composite_power: 0.9,
            zone: "blood_valley",
            breakdown: { karma: -0.35 },
          }),
          createPlayer("FreshFace", {
            composite_power: 0.03,
            zone: "blood_valley",
            breakdown: { karma: 0 },
          }),
        ],
        zones: [createZone("blood_valley", 0.62, { player_count: 2 })],
      }),
    );
    model.setCurrentEra({
      name: "末法纪",
      sinceTick: 100,
      globalEffect: "灵机渐枯，诸域修行更艰",
    });
    model.recordDecision("mutation", {
      commands: [
        {
          type: "modify_zone",
          target: "blood_valley",
          params: { spirit_qi_delta: -0.03 },
        },
      ],
      narrations: [],
      reasoning: "cool down",
    });

    const snapshot = model.toJSON();
    const restored = WorldModel.fromJSON(snapshot);

    expect(restored.currentEra).toEqual(snapshot.currentEra);
    expect(restored.lastTick).toBe(100);
    expect(restored.lastStateTs).toBe(1_710_000_100);
    expect(restored.getZoneHistory("blood_valley")).toEqual(snapshot.zoneHistory.blood_valley);
    expect(restored.getPeerDecisions()).toEqual(model.getPeerDecisions());
    expect(restored.getKeyPlayers().flatMap((player) => player.reasons)).not.toContain("新入世(0.03)");
    expect(snapshot.playerFirstSeenTick).toEqual({
      "offline:Elder": 99,
      "offline:FreshFace": 100,
    });
  });

  it("does not classify restored recurring players as newcomers on the first live tick for legacy snapshots", () => {
    const restored = WorldModel.fromJSON({
      currentEra: null,
      zoneHistory: {},
      lastDecisions: {},
      lastTick: 100,
    });

    restored.updateState(
      createState({
        tick: 101,
        players: [
          createPlayer("Elder", {
            composite_power: 0.88,
            zone: "blood_valley",
            breakdown: { karma: -0.3 },
          }),
        ],
        zones: [createZone("blood_valley", 0.62, { player_count: 1 })],
      }),
    );

    expect(restored.getKeyPlayers().flatMap((player) => player.reasons)).not.toContain("新入世(0.88)");
    expect(restored.toJSON().playerFirstSeenTick).toEqual({
      "offline:Elder": 101,
    });
  });

  it("preserves newcomer detection after restore when player first-seen history is available", () => {
    const model = new WorldModel();
    model.updateState(
      createState({
        tick: 99,
        players: [createPlayer("Elder", { composite_power: 0.88, zone: "blood_valley" })],
        zones: [createZone("blood_valley", 0.66, { player_count: 1 })],
      }),
    );

    const restored = WorldModel.fromJSON(model.toJSON());
    restored.updateState(
      createState({
        tick: 100,
        players: [
          createPlayer("Elder", { composite_power: 0.9, zone: "blood_valley" }),
          createPlayer("FreshFace", { composite_power: 0.03, zone: "blood_valley", breakdown: { karma: 0 } }),
        ],
        zones: [createZone("blood_valley", 0.62, { player_count: 2 })],
      }),
    );

    expect(restored.getKeyPlayers().flatMap((player) => player.reasons)).toContain("新入世(0.03)");
  });

  it("preserves optional faction and disciple summaries in latest state", () => {
    const model = new WorldModel();

    model.updateState({
      v: 1,
      ts: 1_710_000_123,
      tick: 123,
      players: [createPlayer("DiscipleHost")],
      npcs: [
        {
          id: "npc_disciple_001",
          kind: "zombie",
          pos: [1, 64, 1],
          state: "idle",
          blackboard: {},
          digest: {
            archetype: "disciple",
            age_band: "adult",
            age_ratio: 0.55,
            disciple: {
              faction_id: "neutral",
              rank: "disciple",
              loyalty: 0.72,
              lineage: {
                master_id: "npc_master_001",
                disciple_count: 2,
              },
              mission_queue: {
                pending_count: 1,
                top_mission_id: "mission:hold_spawn_gate",
              },
            },
          },
        },
      ],
      factions: [
        { id: "attack", loyalty_bias: 0.5 },
        { id: "defend", loyalty_bias: 0.5 },
        {
          id: "neutral",
          loyalty_bias: 0.7,
          leader_lineage: { disciple_count: 0 },
          mission_queue: { pending_count: 1, top_mission_id: "mission:hold_spawn_gate" },
        },
      ],
      zones: [createZone("starter_zone", 0.5, { player_count: 1 })],
      recent_events: [],
    });

    expect(model.latestState?.factions?.length).toBe(3);
    expect(model.latestState?.npcs[0]?.digest?.disciple?.faction_id).toBe("neutral");
    expect(model.latestState?.npcs[0]?.digest?.disciple?.lineage?.master_id).toBe("npc_master_001");
    expect(model.latestState?.npcs[0]?.digest?.disciple?.mission_queue?.top_mission_id).toBe(
      "mission:hold_spawn_gate",
    );
  });
});
