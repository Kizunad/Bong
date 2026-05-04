import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const CombatRealtimeKindV1 = Type.Union([
  Type.Literal("combat_event"),
  Type.Literal("death_event"),
]);
export type CombatRealtimeKindV1 = Static<typeof CombatRealtimeKindV1>;

export const CombatBodyPartV1 = Type.Union([
  Type.Literal("head"),
  Type.Literal("chest"),
  Type.Literal("abdomen"),
  Type.Literal("arm_l"),
  Type.Literal("arm_r"),
  Type.Literal("leg_l"),
  Type.Literal("leg_r"),
]);
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
    contam_delta: Type.Optional(Type.Number({ minimum: 0 })),
    description: Type.Optional(Type.String({ minLength: 1 })),
    cause: Type.Optional(Type.String({ minLength: 1 })),
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
