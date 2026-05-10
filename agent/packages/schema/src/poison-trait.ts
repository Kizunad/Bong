import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

const EntityId = Type.Integer({ minimum: 0 });

export const PoisonSideEffectTagV1 = Type.Union([
  Type.Literal("qi_focus_drift_2h"),
  Type.Literal("rage_burst_30min"),
  Type.Literal("hallucin_tint_6h"),
  Type.Literal("digest_lock_6h"),
  Type.Literal("toxicity_tier_unlock"),
]);
export type PoisonSideEffectTagV1 = Static<typeof PoisonSideEffectTagV1>;

export const PoisonOverdoseSeverityV1 = Type.Union([
  Type.Literal("mild"),
  Type.Literal("moderate"),
  Type.Literal("severe"),
]);
export type PoisonOverdoseSeverityV1 = Static<typeof PoisonOverdoseSeverityV1>;

export const PoisonDoseEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_entity_id: EntityId,
    dose_amount: Type.Number({ minimum: 0 }),
    side_effect_tag: PoisonSideEffectTagV1,
    poison_level_after: Type.Number({ minimum: 0, maximum: 100 }),
    digestion_after: Type.Number({ minimum: 0 }),
    at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type PoisonDoseEventV1 = Static<typeof PoisonDoseEventV1>;

export const PoisonOverdoseEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_entity_id: EntityId,
    severity: PoisonOverdoseSeverityV1,
    overflow: Type.Number({ minimum: 0 }),
    lifespan_penalty_years: Type.Number({ minimum: 0 }),
    micro_tear_probability: Type.Number({ minimum: 0, maximum: 1 }),
    at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type PoisonOverdoseEventV1 = Static<typeof PoisonOverdoseEventV1>;

export const PoisonTraitStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_entity_id: EntityId,
    poison_toxicity: Type.Number({ minimum: 0, maximum: 100 }),
    digestion_current: Type.Number({ minimum: 0 }),
    digestion_capacity: Type.Number({ exclusiveMinimum: 0 }),
    toxicity_tier_unlocked: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type PoisonTraitStateV1 = Static<typeof PoisonTraitStateV1>;

export function validatePoisonDoseEventV1(data: unknown): ValidationResult {
  return validate(PoisonDoseEventV1, data);
}

export function validatePoisonOverdoseEventV1(data: unknown): ValidationResult {
  return validate(PoisonOverdoseEventV1, data);
}

export function validatePoisonTraitStateV1(data: unknown): ValidationResult {
  return validate(PoisonTraitStateV1, data);
}
