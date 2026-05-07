import { describe, expect, it } from "vitest";
import type { PlayerProfile, WorldStateV1, ZoneSnapshot } from "@bong/schema";
import { createToolContext, validateToolSchema } from "../src/tools/types.js";
import { queryPlayerTool } from "../src/tools/query-player.js";
import { queryPlayerSkillMilestonesTool } from "../src/tools/query-player-skill-milestones.js";
import { queryZoneHistoryTool } from "../src/tools/query-zone-history.js";
import { listActiveEventsTool } from "../src/tools/list-active-events.js";
import { queryRatDensityTool } from "../src/tools/query-rat-density.js";
import { WorldModel } from "../src/world-model.js";

function expectOkResult(value: unknown): asserts value is { ok: true; [key: string]: unknown } {
  expect(value).toBeTruthy();
  expect(typeof value).toBe("object");
  expect((value as { ok?: unknown }).ok).toBe(true);
}

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
  recentEvents?: WorldStateV1["recent_events"];
}): WorldStateV1 {
  const players = args.players ?? [];
  const zones =
    args.zones ?? [createZone("starter_zone", 0.5, { player_count: players.length, active_events: [] })];

  return {
    v: 1,
    ts: 1_710_000_000 + args.tick,
    tick: args.tick,
    season_state: {
      season: "summer",
      tick_into_phase: args.tick,
      phase_total_ticks: 1_382_400,
      year_index: 0,
    },
    players,
    npcs: [],
    rat_density_heatmap: {
      zones: {},
    },
    zones,
    recent_events: args.recentEvents ?? [],
  };
}

function createContextForTools(): ReturnType<typeof createToolContext> {
  const model = new WorldModel();

  model.updateState(
    createState({
      tick: 1,
      players: [
        createPlayer("Veteran", {
          composite_power: 0.9,
          zone: "blood_valley",
          recent_kills: 4,
          breakdown: { karma: -0.4 },
        }),
        createPlayer("FreshFace", {
          composite_power: 0.12,
          zone: "newbie_valley",
        }),
      ],
      zones: [
        createZone("blood_valley", 0.62, {
          danger_level: 3,
          active_events: ["thunder_tribulation", "beast_tide"],
          player_count: 1,
        }),
        createZone("newbie_valley", 0.91, {
          danger_level: 1,
          active_events: ["beast_tide"],
          player_count: 1,
        }),
      ],
    }),
  );

  model.updateState(
    createState({
      tick: 2,
      players: [
        createPlayer("Veteran", {
          composite_power: 0.88,
          zone: "blood_valley",
          recent_kills: 5,
          breakdown: { karma: -0.45 },
        }),
        createPlayer("FreshFace", {
          composite_power: 0.1,
          zone: "newbie_valley",
        }),
      ],
      zones: [
        createZone("blood_valley", 0.58, {
          danger_level: 3,
          active_events: ["thunder_tribulation", "karma_backlash"],
          player_count: 1,
        }),
        createZone("newbie_valley", 0.93, {
          danger_level: 1,
          active_events: ["beast_tide"],
          player_count: 1,
        }),
      ],
      recentEvents: [
        {
          type: "player_join",
          tick: 2,
          player: "offline:FreshFace",
          zone: "newbie_valley",
        },
      ],
    }),
  );

  const latestState = createState({
    tick: 3,
    players: [
      createPlayer("Veteran", {
        composite_power: 0.86,
        zone: "blood_valley",
        recent_kills: 6,
        recent_deaths: 1,
        breakdown: { karma: -0.48, combat: 0.91 },
      }),
      createPlayer("FreshFace", {
        composite_power: 0.08,
        zone: "newbie_valley",
        recent_kills: 0,
        recent_deaths: 0,
      }),
    ],
    zones: [
      createZone("blood_valley", 0.54, {
        danger_level: 4,
        active_events: ["thunder_tribulation", "beast_tide", "beast_tide"],
        player_count: 1,
      }),
      createZone("newbie_valley", 0.95, {
        danger_level: 1,
        active_events: ["beast_tide"],
        player_count: 1,
      }),
      createZone("quiet_lake", 0.7, {
        danger_level: 0,
        active_events: [],
        player_count: 0,
      }),
    ],
    recentEvents: [
      {
        type: "player_join",
        tick: 3,
        player: "offline:FreshFace",
        zone: "newbie_valley",
      },
    ],
  });

  return createToolContext({ latestState, worldModel: model });
}

describe("readonly tools", () => {
  describe("query-player", () => {
    it("returns player profile with protection signals for uuid lookup", async () => {
      const ctx = createContextForTools();
      const result = await queryPlayerTool.execute({ uuid: "offline:FreshFace" }, ctx);

      expect(validateToolSchema(queryPlayerTool.result, result).ok).toBe(true);
      expect(result).toMatchObject({
        ok: true,
        query: { by: "uuid", value: "offline:FreshFace" },
        player: {
          name: "FreshFace",
          zone: "newbie_valley",
          recentKills: 0,
          recentDeaths: 0,
        },
        protection: {
          newbieProtected: true,
          newcomerDetected: true,
          protected: true,
        },
      });
    });

    it("returns structured life record skill milestones when present", async () => {
      const latestState = createState({
        tick: 5,
        players: [
          createPlayer("Veteran", {
            composite_power: 0.86,
            zone: "blood_valley",
            recent_kills: 6,
            recent_deaths: 1,
            breakdown: { karma: -0.48, combat: 0.91 },
            life_record: {
              recent_biography_summary: "t82000:reach:Spirit",
              recent_skill_milestones_summary:
                "t82000:skill:herbalism:lv3 | t83000:skill:alchemy:lv2",
              skill_milestones: [
                {
                  skill: "herbalism",
                  new_lv: 3,
                  achieved_at: 82000,
                  narration: "你摘辨草木渐熟，今已至Lv.3。",
                  total_xp_at: 550,
                },
                {
                  skill: "alchemy",
                  new_lv: 2,
                  achieved_at: 83000,
                  narration: "炉火识性稍深，丹道已至Lv.2。",
                  total_xp_at: 240,
                },
              ],
            },
          }),
        ],
        zones: [
          createZone("blood_valley", 0.54, {
            danger_level: 4,
            active_events: ["thunder_tribulation"],
            player_count: 1,
          }),
        ],
      });
      const ctx = createToolContext({ latestState, worldModel: WorldModel.fromState(latestState) });
      const result = await queryPlayerTool.execute({ uuid: "offline:Veteran" }, ctx);

      expect(validateToolSchema(queryPlayerTool.result, result).ok).toBe(true);
      expect(result).toMatchObject({
        ok: true,
        player: {
          lifeRecord: {
            recentBiographySummary: "t82000:reach:Spirit",
            recentSkillMilestonesSummary:
              "t82000:skill:herbalism:lv3 | t83000:skill:alchemy:lv2",
            recentSkillMilestones: [
              {
                skill: "herbalism",
                newLv: 3,
                achievedAt: 82000,
                totalXpAt: 550,
              },
              {
                skill: "alchemy",
                newLv: 2,
                achievedAt: 83000,
                totalXpAt: 240,
              },
            ],
          },
        },
      });
      expect((result as { summary: string }).summary).toContain("latest skill alchemy Lv.2");
    });

    it("returns structured not-found payload", async () => {
      const ctx = createContextForTools();
      const result = await queryPlayerTool.execute({ name: "Unknown" }, ctx);

      expect(validateToolSchema(queryPlayerTool.result, result).ok).toBe(true);
      expect(result).toEqual({
        ok: false,
        query: { by: "name", value: "Unknown" },
        error: {
          code: "PLAYER_NOT_FOUND",
          message: "player not found by name: Unknown",
        },
      });
    });

    it("returns structured invalid-query payload when both uuid and name are provided", async () => {
      const ctx = createContextForTools();
      const result = await queryPlayerTool.execute(
        { uuid: "offline:Veteran", name: "Veteran" },
        ctx,
      );

      expect(validateToolSchema(queryPlayerTool.result, result).ok).toBe(true);
      expect(result).toEqual({
        ok: false,
        error: {
          code: "INVALID_QUERY",
          message: "'uuid' and 'name' are mutually exclusive",
        },
      });
    });
  });

  describe("query-player-skill-milestones", () => {
    it("returns recent structured milestones with narration text", async () => {
      const latestState = createState({
        tick: 6,
        players: [
          createPlayer("Veteran", {
            composite_power: 0.86,
            zone: "blood_valley",
            life_record: {
              recent_biography_summary: "t82000:reach:Spirit",
              recent_skill_milestones_summary:
                "t82000:skill:herbalism:lv3 | t83000:skill:alchemy:lv2",
              skill_milestones: [
                {
                  skill: "herbalism",
                  new_lv: 3,
                  achieved_at: 82000,
                  narration: "你摘辨草木渐熟，今已至Lv.3。",
                  total_xp_at: 550,
                },
                {
                  skill: "alchemy",
                  new_lv: 2,
                  achieved_at: 83000,
                  narration: "炉火识性稍深，丹道已至Lv.2。",
                  total_xp_at: 240,
                },
              ],
            },
          }),
        ],
        zones: [createZone("blood_valley", 0.54, { player_count: 1 })],
      });
      const ctx = createToolContext({ latestState, worldModel: WorldModel.fromState(latestState) });
      const result = await queryPlayerSkillMilestonesTool.execute(
        { uuid: "offline:Veteran", limit: 1 },
        ctx,
      );

      expect(validateToolSchema(queryPlayerSkillMilestonesTool.result, result).ok).toBe(true);
      expect(result).toEqual({
        ok: true,
        query: { by: "uuid", value: "offline:Veteran" },
        limit: 1,
        player: {
          uuid: "offline:Veteran",
          name: "Veteran",
          zone: "blood_valley",
        },
        milestones: [
          {
            skill: "alchemy",
            newLv: 2,
            achievedAt: 83000,
            narration: "炉火识性稍深，丹道已至Lv.2。",
            totalXpAt: 240,
          },
        ],
        summary: "Veteran@blood_valley recent skill milestones 1",
      });
    });
  });

  describe("query-rat-density", () => {
    it("returns rat phase counts for a known zone", async () => {
      const latestState = createState({
        tick: 12,
        zones: [createZone("blood_valley", 0.71)],
      });
      latestState.rat_density_heatmap.zones.blood_valley = {
        total: 13,
        solitary: 2,
        transitioning: 3,
        gregarious: 8,
      };
      const ctx = createToolContext({
        latestState,
        worldModel: WorldModel.fromState(latestState),
      });

      const result = await queryRatDensityTool.execute({ zone: "blood_valley" }, ctx);

      expect(validateToolSchema(queryRatDensityTool.result, result).ok).toBe(true);
      expect(result).toMatchObject({
        ok: true,
        zone: "blood_valley",
        total: 13,
        dominantPhase: "gregarious",
        phases: {
          solitary: 2,
          transitioning: 3,
          gregarious: 8,
        },
      });
    });

    it("returns structured not-found payload for missing rat density", async () => {
      const ctx = createContextForTools();
      const result = await queryRatDensityTool.execute({ zone: "missing_zone" }, ctx);

      expect(validateToolSchema(queryRatDensityTool.result, result).ok).toBe(true);
      expect(result).toEqual({
        ok: false,
        zone: "missing_zone",
        error: {
          code: "ZONE_NOT_FOUND",
          message: "rat density for zone 'missing_zone' not found",
        },
      });
    });
  });

  describe("query-zone-history", () => {
    it("returns bounded history and trend summary", async () => {
      const ctx = createContextForTools();
      const result = await queryZoneHistoryTool.execute({ zone: "blood_valley", limit: 2 }, ctx);

      expect(validateToolSchema(queryZoneHistoryTool.result, result).ok).toBe(true);
      expect(result).toMatchObject({
        ok: true,
        zone: "blood_valley",
        limit: 2,
        trend: {
          direction: "falling",
        },
      });
      expectOkResult(result);
      const history = result.history as unknown[];
      expect(history).toHaveLength(2);
    });

    it("returns structured not-found payload when zone history is absent", async () => {
      const ctx = createContextForTools();
      const result = await queryZoneHistoryTool.execute({ zone: "missing_zone" }, ctx);

      expect(validateToolSchema(queryZoneHistoryTool.result, result).ok).toBe(true);
      expect(result).toEqual({
        ok: false,
        zone: "missing_zone",
        limit: 5,
        error: {
          code: "ZONE_NOT_FOUND",
          message: "zone 'missing_zone' has no history",
        },
      });
    });

    it("returns structured invalid-query payload for empty zone", async () => {
      const ctx = createContextForTools();
      const result = await queryZoneHistoryTool.execute({ zone: "   " }, ctx);

      expect(validateToolSchema(queryZoneHistoryTool.result, result).ok).toBe(true);
      expect(result).toEqual({
        ok: false,
        error: {
          code: "INVALID_QUERY",
          message: "'zone' must be a non-empty string",
        },
      });
    });
  });

  describe("list-active-events", () => {
    it("returns per-zone active events with deduplicated summary", async () => {
      const ctx = createContextForTools();
      const result = await listActiveEventsTool.execute({}, ctx);

      expect(validateToolSchema(listActiveEventsTool.result, result).ok).toBe(true);
      expect(result).toMatchObject({
        ok: true,
        dedupedEvents: ["beast_tide", "thunder_tribulation"],
      });
      expectOkResult(result);
      const zones = result.zones as Array<{ name: string }>;
      expect(zones.map((zone) => zone.name)).toEqual([
        "blood_valley",
        "newbie_valley",
        "quiet_lake",
      ]);
    });

    it("returns structured not-found payload for missing zone filter", async () => {
      const ctx = createContextForTools();
      const result = await listActiveEventsTool.execute({ zone: "missing_zone" }, ctx);

      expect(validateToolSchema(listActiveEventsTool.result, result).ok).toBe(true);
      expect(result).toEqual({
        ok: false,
        error: {
          code: "ZONE_NOT_FOUND",
          message: "zone 'missing_zone' not found",
        },
      });
    });

    it("returns structured invalid-query payload for empty zone filter", async () => {
      const ctx = createContextForTools();
      const result = await listActiveEventsTool.execute({ zone: "   " }, ctx);

      expect(validateToolSchema(listActiveEventsTool.result, result).ok).toBe(true);
      expect(result).toEqual({
        ok: false,
        error: {
          code: "INVALID_QUERY",
          message: "'zone' must be a non-empty string when provided",
        },
      });
    });
  });
});
