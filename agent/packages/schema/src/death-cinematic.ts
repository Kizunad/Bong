import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const DeathCinematicPhaseV1 = Type.Union([
  Type.Literal("predeath"),
  Type.Literal("death_moment"),
  Type.Literal("roll"),
  Type.Literal("insight_overlay"),
  Type.Literal("darkness"),
  Type.Literal("rebirth"),
]);
export type DeathCinematicPhaseV1 = Static<typeof DeathCinematicPhaseV1>;

export const DeathRollResultV1 = Type.Union([
  Type.Literal("pending"),
  Type.Literal("survive"),
  Type.Literal("fall"),
  Type.Literal("final"),
]);
export type DeathRollResultV1 = Static<typeof DeathRollResultV1>;

export const DeathCinematicZoneKindV1 = Type.Union([
  Type.Literal("ordinary"),
  Type.Literal("death"),
  Type.Literal("negative"),
]);
export type DeathCinematicZoneKindV1 = Static<typeof DeathCinematicZoneKindV1>;

export const DeathCinematicRollV1 = Type.Object(
  {
    probability: Type.Number({ minimum: 0, maximum: 1 }),
    threshold: Type.Number({ minimum: 0, maximum: 1 }),
    luck_value: Type.Number({ minimum: 0, maximum: 1 }),
    result: DeathRollResultV1,
  },
  { additionalProperties: false },
);
export type DeathCinematicRollV1 = Static<typeof DeathCinematicRollV1>;

export const DeathCinematicS2cV1 = Type.Object(
  {
    v: Type.Literal(1),
    character_id: Type.String({ minLength: 1, maxLength: 160 }),
    phase: DeathCinematicPhaseV1,
    phase_tick: Type.Integer({ minimum: 0 }),
    phase_duration_ticks: Type.Integer({ minimum: 1 }),
    total_elapsed_ticks: Type.Integer({ minimum: 0 }),
    total_duration_ticks: Type.Integer({ minimum: 1 }),
    roll: DeathCinematicRollV1,
    insight_text: Type.Array(Type.String({ maxLength: 500 }), { maxItems: 24 }),
    is_final: Type.Boolean(),
    death_number: Type.Integer({ minimum: 1 }),
    zone_kind: DeathCinematicZoneKindV1,
    tsy_death: Type.Boolean(),
    rebirth_weakened_ticks: Type.Integer({ minimum: 0 }),
    skip_predeath: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type DeathCinematicS2cV1 = Static<typeof DeathCinematicS2cV1>;

export function validateDeathCinematicS2cV1Contract(data: unknown): ValidationResult {
  return validate(DeathCinematicS2cV1, data);
}
