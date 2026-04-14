import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const CombatRealtimeKindV1 = Type.Union([
  Type.Literal("combat_event"),
  Type.Literal("death_event"),
]);
export type CombatRealtimeKindV1 = Static<typeof CombatRealtimeKindV1>;

export const CombatRealtimeEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: CombatRealtimeKindV1,
    tick: Type.Integer({ minimum: 0 }),
    target_id: Type.String({ minLength: 1 }),
    attacker_id: Type.Optional(Type.String({ minLength: 1 })),
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
