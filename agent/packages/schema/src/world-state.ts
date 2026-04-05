import { Type, type Static } from "@sinclair/typebox";
import { GameEventType, NpcState, PlayerTrend } from "./common.js";

// ─── 子结构 ─────────────────────────────────────────────

export const Vec3 = Type.Tuple([Type.Number(), Type.Number(), Type.Number()]);
export type Vec3 = Static<typeof Vec3>;

export const PlayerPowerBreakdown = Type.Object({
  combat: Type.Number({ minimum: 0, maximum: 1 }),
  wealth: Type.Number({ minimum: 0, maximum: 1 }),
  social: Type.Number({ minimum: 0, maximum: 1 }),
  karma: Type.Number({ minimum: -1, maximum: 1 }),
  territory: Type.Number({ minimum: 0, maximum: 1 }),
});
export type PlayerPowerBreakdown = Static<typeof PlayerPowerBreakdown>;

export const PlayerProfile = Type.Object({
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
});
export type PlayerProfile = Static<typeof PlayerProfile>;

export const NpcSnapshot = Type.Object({
  id: Type.String(),
  kind: Type.String(),
  pos: Vec3,
  state: NpcState,
  blackboard: Type.Record(Type.String(), Type.Any()),
});
export type NpcSnapshot = Static<typeof NpcSnapshot>;

export const ZoneSnapshot = Type.Object({
  name: Type.String(),
  spirit_qi: Type.Number({ minimum: 0, maximum: 1 }),
  danger_level: Type.Integer({ minimum: 0, maximum: 5 }),
  active_events: Type.Array(Type.String()),
  player_count: Type.Integer({ minimum: 0 }),
});
export type ZoneSnapshot = Static<typeof ZoneSnapshot>;

export const GameEvent = Type.Object({
  type: GameEventType,
  tick: Type.Integer(),
  player: Type.Optional(Type.String()),
  target: Type.Optional(Type.String()),
  zone: Type.Optional(Type.String()),
  details: Type.Optional(Type.Record(Type.String(), Type.Any())),
});
export type GameEvent = Static<typeof GameEvent>;

// ─── 顶层消息 ──────────────────────────────────────────

export const WorldStateV1 = Type.Object({
  v: Type.Literal(1),
  ts: Type.Integer({ description: "Unix timestamp (seconds)" }),
  tick: Type.Integer({ description: "Server game tick" }),
  players: Type.Array(PlayerProfile),
  npcs: Type.Array(NpcSnapshot),
  zones: Type.Array(ZoneSnapshot),
  recent_events: Type.Array(GameEvent),
});
export type WorldStateV1 = Static<typeof WorldStateV1>;
