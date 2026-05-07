/**
 * Mock world state for testing without server connection
 */

import type { WorldStateV1 } from "@bong/schema";

export function createMockWorldState(): WorldStateV1 {
  return {
    v: 1,
    ts: Math.floor(Date.now() / 1000),
    tick: 84000,
    season_state: {
      season: "summer",
      tick_into_phase: 84000,
      phase_total_ticks: 1_382_400,
      year_index: 0,
    },
    players: [
      {
        uuid: "offline:Steve",
        name: "Steve",
        realm: "Induce",
        composite_power: 0.85,
        breakdown: { combat: 0.92, wealth: 0.60, social: 0.45, karma: -0.45, territory: 0.20 },
        trend: "rising",
        active_hours: 12.5,
        zone: "blood_valley",
        pos: [100, 66, 100],
        recent_kills: 8,
        recent_deaths: 1,
      },
      {
        uuid: "offline:Alex",
        name: "Alex",
        realm: "Condense",
        composite_power: 0.35,
        breakdown: { combat: 0.20, wealth: 0.40, social: 0.65, karma: 0.30, territory: 0.10 },
        trend: "stable",
        active_hours: 6.0,
        zone: "green_cloud_peak",
        pos: [50, 72, 50],
        recent_kills: 0,
        recent_deaths: 2,
      },
      {
        uuid: "offline:NewPlayer1",
        name: "NewPlayer1",
        realm: "Awaken",
        composite_power: 0.08,
        breakdown: { combat: 0.05, wealth: 0.10, social: 0.05, karma: 0.00, territory: 0.00 },
        trend: "rising",
        active_hours: 1.0,
        zone: "newbie_valley",
        pos: [8, 66, 8],
        recent_kills: 0,
        recent_deaths: 0,
      },
    ],
    npcs: [
      {
        id: "npc_zombie_001",
        kind: "zombie",
        zone: "blood_valley",
        pos: [14, 66, 14],
        state: "idle",
        blackboard: { nearest_player: null, player_distance: 999 },
      },
    ],
    zones: [
      { name: "blood_valley", spirit_qi: 0.42, danger_level: 3, active_events: [], player_count: 1 },
      { name: "green_cloud_peak", spirit_qi: 0.88, danger_level: 1, active_events: [], player_count: 1 },
      { name: "newbie_valley", spirit_qi: 0.95, danger_level: 0, active_events: [], player_count: 1 },
    ],
    rat_density_heatmap: {
      zones: {},
    },
    recent_events: [
      { type: "player_kill_npc", tick: 83200, player: "offline:Steve", zone: "blood_valley" },
      { type: "player_kill_npc", tick: 83500, player: "offline:Steve", zone: "blood_valley" },
      { type: "player_death", tick: 83800, player: "offline:Alex", zone: "green_cloud_peak" },
    ],
  };
}
