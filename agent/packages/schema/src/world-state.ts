import { Type, type Static } from "@sinclair/typebox";

import { GameEventType, NpcState, PlayerTrend } from "./common.js";
import { CultivationSnapshotV1, LifeRecordSnapshotV1 } from "./cultivation.js";
import { PlayerSocialSnapshotV1 } from "./social.js";
import { validate, type ValidationResult } from "./validate.js";

// ─── 子结构 ─────────────────────────────────────────────

export const Vec3 = Type.Tuple([Type.Number(), Type.Number(), Type.Number()]);
export type Vec3 = Static<typeof Vec3>;

export const FactionId = Type.Union([
  Type.Literal("attack"),
  Type.Literal("defend"),
  Type.Literal("neutral"),
]);
export type FactionId = Static<typeof FactionId>;

export const FactionRank = Type.Union([
  Type.Literal("leader"),
  Type.Literal("disciple"),
  Type.Literal("ally"),
]);
export type FactionRank = Static<typeof FactionRank>;

export const LineageSummaryV1 = Type.Object(
  {
    master_id: Type.Optional(Type.String()),
    disciple_count: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type LineageSummaryV1 = Static<typeof LineageSummaryV1>;

export const MissionQueueSummaryV1 = Type.Object(
  {
    pending_count: Type.Integer({ minimum: 0 }),
    top_mission_id: Type.Optional(Type.String()),
  },
  { additionalProperties: false },
);
export type MissionQueueSummaryV1 = Static<typeof MissionQueueSummaryV1>;

export const DiscipleSummaryV1 = Type.Object(
  {
    faction_id: FactionId,
    rank: FactionRank,
    loyalty: Type.Number({ minimum: 0, maximum: 1 }),
    lineage: Type.Optional(LineageSummaryV1),
    mission_queue: Type.Optional(MissionQueueSummaryV1),
  },
  { additionalProperties: false },
);
export type DiscipleSummaryV1 = Static<typeof DiscipleSummaryV1>;

export const FactionSummaryV1 = Type.Object(
  {
    id: FactionId,
    loyalty_bias: Type.Number({ minimum: 0, maximum: 1 }),
    leader_lineage: Type.Optional(LineageSummaryV1),
    mission_queue: Type.Optional(MissionQueueSummaryV1),
  },
  { additionalProperties: false },
);
export type FactionSummaryV1 = Static<typeof FactionSummaryV1>;

export const PlayerPowerBreakdown = Type.Object(
  {
    combat: Type.Number({ minimum: 0, maximum: 1 }),
    wealth: Type.Number({ minimum: 0, maximum: 1 }),
    social: Type.Number({ minimum: 0, maximum: 1 }),
    karma: Type.Number({ minimum: -1, maximum: 1 }),
    territory: Type.Number({ minimum: 0, maximum: 1 }),
  },
  { additionalProperties: false },
);
export type PlayerPowerBreakdown = Static<typeof PlayerPowerBreakdown>;

export const ZoneStatusV1 = Type.Union([
  Type.Literal("normal"),
  Type.Literal("collapsed"),
]);
export type ZoneStatusV1 = Static<typeof ZoneStatusV1>;

export const SeasonV1 = Type.Union([
  Type.Literal("summer"),
  Type.Literal("summer_to_winter"),
  Type.Literal("winter"),
  Type.Literal("winter_to_summer"),
]);
export type SeasonV1 = Static<typeof SeasonV1>;

export const SeasonStateV1 = Type.Object(
  {
    season: SeasonV1,
    tick_into_phase: Type.Integer({ minimum: 0 }),
    phase_total_ticks: Type.Integer({ minimum: 1 }),
    year_index: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SeasonStateV1 = Static<typeof SeasonStateV1>;

export const PlayerProfile = Type.Object(
  {
    uuid: Type.String(),
    name: Type.String(),
    realm: Type.String(),
    composite_power: Type.Number({ minimum: 0, maximum: 1 }),
    breakdown: PlayerPowerBreakdown,
    trend: PlayerTrend,
    active_hours: Type.Number(),
    zone: Type.String(),
    pos: Vec3,
    recent_kills: Type.Integer({ minimum: 0 }),
    recent_deaths: Type.Integer({ minimum: 0 }),
    cultivation: Type.Optional(CultivationSnapshotV1),
    life_record: Type.Optional(LifeRecordSnapshotV1),
    social: Type.Optional(PlayerSocialSnapshotV1),
  },
  { additionalProperties: false },
);
export type PlayerProfile = Static<typeof PlayerProfile>;

export const NpcSnapshot = Type.Object(
  {
    id: Type.String(),
    kind: Type.String(),
    zone: Type.String(),
    pos: Vec3,
    state: NpcState,
    blackboard: Type.Record(Type.String(), Type.Any()),
    digest: Type.Optional(
      Type.Object(
        {
          archetype: Type.String(),
          age_band: Type.String(),
          age_ratio: Type.Number({ minimum: 0, maximum: 1 }),
          realm: Type.Optional(Type.String()),
          faction_id: Type.Optional(FactionId),
          position: Type.Optional(Vec3),
          disciple: Type.Optional(DiscipleSummaryV1),
        },
        { additionalProperties: false },
      ),
    ),
  },
  { additionalProperties: false },
);
export type NpcSnapshot = Static<typeof NpcSnapshot>;

export const ZoneSnapshot = Type.Object(
  {
    name: Type.String(),
    spirit_qi: Type.Number({ minimum: -1, maximum: 1 }),
    danger_level: Type.Integer({ minimum: 0, maximum: 5 }),
    status: Type.Optional(ZoneStatusV1),
    active_events: Type.Array(Type.String()),
    player_count: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ZoneSnapshot = Static<typeof ZoneSnapshot>;

export const GameEvent = Type.Object(
  {
    type: GameEventType,
    tick: Type.Integer(),
    player: Type.Optional(Type.String()),
    target: Type.Optional(Type.String()),
    zone: Type.Optional(Type.String()),
    details: Type.Optional(Type.Record(Type.String(), Type.Any())),
  },
  { additionalProperties: false },
);
export type GameEvent = Static<typeof GameEvent>;

export const RatDensitySnapshotV1 = Type.Object(
  {
    total: Type.Integer({ minimum: 0 }),
    solitary: Type.Integer({ minimum: 0 }),
    transitioning: Type.Integer({ minimum: 0 }),
    gregarious: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type RatDensitySnapshotV1 = Static<typeof RatDensitySnapshotV1>;

export const RatDensityHeatmapV1 = Type.Object(
  {
    zones: Type.Record(Type.String(), RatDensitySnapshotV1),
  },
  { additionalProperties: false },
);
export type RatDensityHeatmapV1 = Static<typeof RatDensityHeatmapV1>;

// ─── 顶层消息 ──────────────────────────────────────────

export const WorldStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    ts: Type.Integer({ description: "Unix timestamp (seconds)" }),
    tick: Type.Integer({ description: "Server game tick" }),
    season_state: SeasonStateV1,
    players: Type.Array(PlayerProfile),
    npcs: Type.Array(NpcSnapshot),
    factions: Type.Optional(Type.Array(FactionSummaryV1)),
    rat_density_heatmap: RatDensityHeatmapV1,
    zones: Type.Array(ZoneSnapshot),
    recent_events: Type.Array(GameEvent),
  },
  { additionalProperties: false },
);
export type WorldStateV1 = Static<typeof WorldStateV1>;

export function validateWorldStateV1Contract(data: unknown): ValidationResult {
  return validate(WorldStateV1, data);
}
