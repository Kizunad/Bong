import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const CombatRealtimeKindV1 = Type.Union([
  Type.Literal("combat_event"),
  Type.Literal("death_event"),
]);
export type CombatRealtimeKindV1 = Static<typeof CombatRealtimeKindV1>;

export const CombatDefenseKindV1 = Type.Union([
  Type.Literal("jie_mai"),
  Type.Literal("sword_parry"),
  // plan-shield-block-v1 P2 — 凡人盾格挡（无境界加成，无截脉窗口）
  Type.Literal("shield_block"),
]);
export type CombatDefenseKindV1 = Static<typeof CombatDefenseKindV1>;

/**
 * 命中部位 id（plan-race-system-v1 P1c）。
 *
 * 曾是 8 段人形闭合 union，wire 开放化后改为任意 string part id（humanoid 沿用 8 个
 * 既有 snake_case 名字，非 humanoid 构型如 P5 飞鲸的部位不在这 8 段之列）。不留
 * dual-form 兼容层。
 */
export const CombatBodyPartV1 = Type.String({
  minLength: 1,
  description: "命中部位 id（humanoid 沿用 8 段既有名字，非 humanoid 构型可为任意 id）",
});
export type CombatBodyPartV1 = Static<typeof CombatBodyPartV1>;

export const CombatWoundKindV1 = Type.Union([
  Type.Literal("cut"),
  Type.Literal("blunt"),
  Type.Literal("pierce"),
  Type.Literal("burn"),
  Type.Literal("concussion"),
]);
export type CombatWoundKindV1 = Static<typeof CombatWoundKindV1>;

export const CombatAttackSourceV1 = Type.Union([
  Type.Literal("melee"),
  Type.Literal("burst_meridian"),
  Type.Literal("qi_needle"),
  Type.Literal("full_power"),
  Type.Literal("sword_cleave"),
  Type.Literal("sword_thrust"),
  // plan-sword-path-complete §E — 剑道五招专属变体（追加，保持旧 6 个顺序不变 → 向后兼容）
  Type.Literal("sword_path_condense_edge"),
  Type.Literal("sword_path_qi_slash"),
  Type.Literal("sword_path_resonance"),
  Type.Literal("sword_path_manifest"),
  Type.Literal("sword_path_heaven_gate"),
]);
export type CombatAttackSourceV1 = Static<typeof CombatAttackSourceV1>;

export const CombatRealtimeEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: CombatRealtimeKindV1,
    tick: Type.Integer({ minimum: 0 }),
    target_id: Type.String({ minLength: 1 }),
    attacker_id: Type.Optional(Type.String({ minLength: 1 })),
    body_part: Type.Optional(CombatBodyPartV1),
    wound_kind: Type.Optional(CombatWoundKindV1),
    source: Type.Optional(CombatAttackSourceV1),
    damage: Type.Optional(Type.Number({ minimum: 0 })),
    physical_damage: Type.Optional(Type.Number({ minimum: 0 })),
    contam_delta: Type.Optional(Type.Number({ minimum: 0 })),
    description: Type.Optional(Type.String({ minLength: 1 })),
    cause: Type.Optional(Type.String({ minLength: 1 })),
    defense_kind: Type.Optional(CombatDefenseKindV1),
    defense_effectiveness: Type.Optional(Type.Number({ minimum: 0.3, maximum: 1 })),
    defense_contam_reduced: Type.Optional(Type.Number({ minimum: 0 })),
    defense_wound_severity: Type.Optional(Type.Number({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type CombatRealtimeEventV1 = Static<typeof CombatRealtimeEventV1>;

export const CombatSummaryV1 = Type.Object(
  {
    v: Type.Literal(1),
    window_start_tick: Type.Integer({ minimum: 0 }),
    window_end_tick: Type.Integer({ minimum: 0 }),
    combat_event_count: Type.Integer({ minimum: 0 }),
    death_event_count: Type.Integer({ minimum: 0 }),
    damage_total: Type.Number({ minimum: 0 }),
    contam_delta_total: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type CombatSummaryV1 = Static<typeof CombatSummaryV1>;

export function validateCombatRealtimeEventV1Contract(
  data: unknown,
): ValidationResult {
  return validate(CombatRealtimeEventV1, data);
}

export function validateCombatSummaryV1Contract(data: unknown): ValidationResult {
  return validate(CombatSummaryV1, data);
}
