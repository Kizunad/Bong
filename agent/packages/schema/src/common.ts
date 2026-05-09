import { Type, type Static } from "@sinclair/typebox";

// ─── 共享常量 ───────────────────────────────────────────

/** 全服灵气守恒总量 */
export const SPIRIT_QI_TOTAL = 100.0;

/** 指令 intensity 范围 */
export const INTENSITY_MIN = 0.0;
export const INTENSITY_MAX = 1.0;

/** 同一目标冷却时间 (ms) */
export const COOLDOWN_SAME_TARGET_MS = 600_000;

/** 新手保护阈值 */
export const NEWBIE_POWER_THRESHOLD = 0.2;

/** 单轮最大指令数 */
export const MAX_COMMANDS_PER_TICK = 5;

/** 单条叙事文本最大字符数 */
export const MAX_NARRATION_LENGTH = 500;

/** 单条 payload 最大字节数 (CustomPayload S2C) */
export const MAX_PAYLOAD_BYTES = 8192;

// ─── 共享枚举 ───────────────────────────────────────────

export const CommandType = Type.Union([
  Type.Literal("spawn_event"),
  Type.Literal("spawn_npc"),
  Type.Literal("despawn_npc"),
  Type.Literal("faction_event"),
  Type.Literal("modify_zone"),
  Type.Literal("npc_behavior"),
]);
export type CommandType = Static<typeof CommandType>;

export const EventKind = Type.Union([
  Type.Literal("thunder_tribulation"),
  Type.Literal("beast_tide"),
  Type.Literal("realm_collapse"),
  Type.Literal("karma_backlash"),
]);
export type EventKind = Static<typeof EventKind>;

export const NarrationScope = Type.Union([
  Type.Literal("broadcast"),
  Type.Literal("zone"),
  Type.Literal("player"),
]);
export type NarrationScope = Static<typeof NarrationScope>;

export const NarrationStyle = Type.Union([
  Type.Literal("system_warning"),
  Type.Literal("perception"),
  Type.Literal("narration"),
  Type.Literal("era_decree"),
  Type.Literal("political_jianghu"),
]);
export type NarrationStyle = Static<typeof NarrationStyle>;

export const NarrationKind = Type.Union([
  Type.Literal("death_insight"),
  Type.Literal("niche_intrusion"),
  Type.Literal("niche_intrusion_by_npc"),
  Type.Literal("npc_farm_pressure"),
  Type.Literal("scattered_cultivator"),
  Type.Literal("political_jianghu"),
]);
export type NarrationKind = Static<typeof NarrationKind>;

export const ChatIntent = Type.Union([
  Type.Literal("complaint"),
  Type.Literal("boast"),
  Type.Literal("social"),
  Type.Literal("help"),
  Type.Literal("provoke"),
  Type.Literal("unknown"),
]);
export type ChatIntent = Static<typeof ChatIntent>;

export const PlayerTrend = Type.Union([
  Type.Literal("rising"),
  Type.Literal("stable"),
  Type.Literal("falling"),
]);
export type PlayerTrend = Static<typeof PlayerTrend>;

export const NpcState = Type.Union([
  Type.Literal("idle"),
  Type.Literal("fleeing"),
  Type.Literal("attacking"),
  Type.Literal("patrolling"),
]);
export type NpcState = Static<typeof NpcState>;

export const GameEventType = Type.Union([
  Type.Literal("player_kill_npc"),
  Type.Literal("player_kill_player"),
  Type.Literal("player_death"),
  Type.Literal("npc_spawn"),
  Type.Literal("zone_qi_change"),
  Type.Literal("event_triggered"),
  Type.Literal("player_join"),
  Type.Literal("player_leave"),
]);
export type GameEventType = Static<typeof GameEventType>;
