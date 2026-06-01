import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const NpcArchetypeV1 = Type.Union([
  Type.Literal("zombie"),
  Type.Literal("commoner"),
  Type.Literal("rogue"),
  Type.Literal("beast"),
  Type.Literal("disciple"),
  Type.Literal("guardian_relic"),
  Type.Literal("daoxiang"),
  Type.Literal("zhinian"),
  Type.Literal("fuya"),
  Type.Literal("skull_fiend"),
]);
export type NpcArchetypeV1 = Static<typeof NpcArchetypeV1>;

export const NpcSpawnSourceV1 = Type.Union([
  Type.Literal("startup"),
  Type.Literal("seed"),
  Type.Literal("reproduction"),
  Type.Literal("agent_command"),
]);
export type NpcSpawnSourceV1 = Static<typeof NpcSpawnSourceV1>;

export const NpcDeathCauseV1 = Type.Union([
  Type.Literal("natural_aging"),
  Type.Literal("combat"),
  Type.Literal("despawned"),
  Type.Literal("duo_she"),
]);
export type NpcDeathCauseV1 = Static<typeof NpcDeathCauseV1>;

export const FactionIdV1 = Type.Union([
  Type.Literal("attack"),
  Type.Literal("defend"),
  Type.Literal("neutral"),
]);
export type FactionIdV1 = Static<typeof FactionIdV1>;

export const FactionEventKindV1 = Type.Union([
  Type.Literal("set_leader"),
  Type.Literal("clear_leader"),
  Type.Literal("set_leader_lineage"),
  Type.Literal("adjust_loyalty_bias"),
  Type.Literal("enqueue_mission"),
  Type.Literal("pop_mission"),
]);
export type FactionEventKindV1 = Static<typeof FactionEventKindV1>;

export const NpcSpawnedV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("npc_spawned"),
    npc_id: Type.String({ minLength: 1 }),
    archetype: NpcArchetypeV1,
    source: NpcSpawnSourceV1,
    zone: Type.String({ minLength: 1 }),
    pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    initial_age_ticks: Type.Number({ minimum: 0 }),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type NpcSpawnedV1 = Static<typeof NpcSpawnedV1>;

export const NpcDeathV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("npc_death"),
    npc_id: Type.String({ minLength: 1 }),
    archetype: NpcArchetypeV1,
    cause: NpcDeathCauseV1,
    faction_id: Type.Optional(FactionIdV1),
    life_record_snapshot: Type.Optional(Type.String()),
    age_ticks: Type.Number({ minimum: 0 }),
    max_age_ticks: Type.Number({ minimum: 0 }),
    at_tick: Type.Number({ minimum: 0 }),
    // plan-offscreen-war-v1 P0：是否离屏 dormant 派系互殴所致。
    // 字段名匹配 server serde wire key（snake_case，无 rename）。Optional 让旧
    // payload（无此字段）仍过校验，新 server 永远带 false/true。
    from_dormant_combat: Type.Optional(Type.Boolean()),
    // plan-offscreen-war-v1 P0：死亡坐标 [x,y,z]，server pos=None 时不上线 → Optional。
    pos: Type.Optional(Type.Tuple([Type.Number(), Type.Number(), Type.Number()])),
  },
  { additionalProperties: false },
);
export type NpcDeathV1 = Static<typeof NpcDeathV1>;

export const FactionEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("faction_event"),
    faction_id: FactionIdV1,
    event_kind: FactionEventKindV1,
    leader_id: Type.Optional(Type.String({ minLength: 1 })),
    loyalty_bias: Type.Number({ minimum: 0, maximum: 1 }),
    mission_queue_size: Type.Integer({ minimum: 0 }),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FactionEventV1 = Static<typeof FactionEventV1>;

/**
 * plan-offscreen-war-v1 P5：散修群体消长三态（reframe b）。
 * rising / stable / waning 与 server GroupStatus snake_case wire 一一对应。
 */
export const GroupStatusV1 = Type.Union([
  Type.Literal("rising"),
  Type.Literal("stable"),
  Type.Literal("waning"),
]);
export type GroupStatusV1 = Static<typeof GroupStatusV1>;

/**
 * plan-offscreen-war-v1 P5：散修群体消长盘面 telemetry（`bong:faction_state`，reframe b）。
 *
 * 逐字段对齐 server `FactionStateV1` serde：
 * - `v`：u8 序列化为数字 → Literal(1)
 * - `group_id`：u16 → Integer({ minimum: 0 })（TypeBox Integer 不限 max，上层合规校验由 server 保证）
 * - `population`：u32 → Integer({ minimum: 0 })
 * - `status`：GroupStatus snake_case enum → GroupStatusV1 union
 * - `at_tick`：u64 → Number({ minimum: 0 })（JS 安全整数范围内的 u64）
 * - `additionalProperties: false` 与 server serde `deny_unknown_fields` 等价
 */
export const FactionStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("faction_state"),
    group_id: Type.Integer({ minimum: 0 }),
    region_descriptor: Type.String({ minLength: 1 }),
    population: Type.Integer({ minimum: 0 }),
    status: GroupStatusV1,
    dominant_zone: Type.String({ minLength: 1 }),
    strongest_realm: Type.String(),
    strongest_char_id: Type.String(),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FactionStateV1 = Static<typeof FactionStateV1>;

export function validateNpcSpawnedV1Contract(data: unknown): ValidationResult {
  return validate(NpcSpawnedV1, data);
}

export function validateNpcDeathV1Contract(data: unknown): ValidationResult {
  return validate(NpcDeathV1, data);
}

export function validateFactionEventV1Contract(data: unknown): ValidationResult {
  return validate(FactionEventV1, data);
}

export function validateFactionStateV1Contract(data: unknown): ValidationResult {
  return validate(FactionStateV1, data);
}

/**
 * plan-offscreen-war-v1 P9：涌现冲突阶段（snake_case，对齐 server WarPhase serde）。
 */
export const WarPhaseV1 = Type.Union([
  Type.Literal("emerging"),
  Type.Literal("skirmish"),
  Type.Literal("settling"),
  Type.Literal("aftermath"),
]);
export type WarPhaseV1 = Static<typeof WarPhaseV1>;

/**
 * plan-offscreen-war-v1 P9：涌现区域冲突 telemetry 事件（`bong:faction/war`）。
 *
 * 逐字段对齐 server `FactionWarEventV1` serde（schema/npc.rs:140）：
 * - `v`：u8 → Literal(1)
 * - `kind`：固定 "faction_war_event"
 * - `war_id`：u64 → Number({ minimum: 0 })
 * - `zone`：String
 * - `region_descriptor`：匿名描述符（无具名宗门）
 * - `phase`：WarPhaseV1
 * - `groups`：Array of u16 group_id
 * - `winner_group` / `loser_group`：Optional u16（Settling/Aftermath 才 Some）
 * - `total_casualties`：Optional u32
 * - `at_tick`：u64 → Number({ minimum: 0 })
 * - 守恒红线：**不含任何真元字段**（零真元）
 */
export const FactionWarEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("faction_war_event"),
    war_id: Type.Number({ minimum: 0 }),
    zone: Type.String({ minLength: 1 }),
    region_descriptor: Type.String({ minLength: 1 }),
    phase: WarPhaseV1,
    groups: Type.Array(Type.Integer({ minimum: 0 }), { minItems: 0 }),
    enlist_count: Type.Integer({ minimum: 0 }),
    mercenary_count: Type.Integer({ minimum: 0 }),
    intercept_count: Type.Integer({ minimum: 0 }),
    spectate_count: Type.Integer({ minimum: 0 }),
    winner_group: Type.Optional(Type.Integer({ minimum: 0 })),
    loser_group: Type.Optional(Type.Integer({ minimum: 0 })),
    total_casualties: Type.Optional(Type.Integer({ minimum: 0 })),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FactionWarEventV1 = Static<typeof FactionWarEventV1>;

export function validateFactionWarEventV1Contract(data: unknown): ValidationResult {
  return validate(FactionWarEventV1, data);
}
