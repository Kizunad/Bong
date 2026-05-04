import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const CarrierKindV1 = Type.Union([Type.Literal("yibian_shougu")]);
export type CarrierKindV1 = Static<typeof CarrierKindV1>;

export const CarrierChargePhaseV1 = Type.Union([
  Type.Literal("idle"),
  Type.Literal("charging"),
  Type.Literal("charged"),
]);
export type CarrierChargePhaseV1 = Static<typeof CarrierChargePhaseV1>;

export const CarrierStateV1 = Type.Object(
  {
    carrier: Type.String({ minLength: 1 }),
    phase: CarrierChargePhaseV1,
    progress: Type.Number({ minimum: 0, maximum: 1 }),
    sealed_qi: Type.Number({ minimum: 0 }),
    sealed_qi_initial: Type.Number({ minimum: 0 }),
    half_life_remaining_ticks: Type.Integer({ minimum: 0 }),
    item_instance_id: Type.Optional(Type.Integer({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type CarrierStateV1 = Static<typeof CarrierStateV1>;

export const CarrierChargedEventV1 = Type.Object(
  {
    carrier: Type.String({ minLength: 1 }),
    instance_id: Type.Integer({ minimum: 0 }),
    qi_amount: Type.Number({ minimum: 0 }),
    qi_color: Type.String({ minLength: 1 }),
    full_charge: Type.Boolean(),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type CarrierChargedEventV1 = Static<typeof CarrierChargedEventV1>;

export const CarrierImpactEventV1 = Type.Object(
  {
    attacker: Type.String({ minLength: 1 }),
    target: Type.String({ minLength: 1 }),
    carrier_kind: CarrierKindV1,
    hit_distance: Type.Number({ minimum: 0 }),
    sealed_qi_initial: Type.Number({ minimum: 0 }),
    hit_qi: Type.Number({ minimum: 0 }),
    wound_damage: Type.Number({ minimum: 0 }),
    contam_amount: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type CarrierImpactEventV1 = Static<typeof CarrierImpactEventV1>;

export const ProjectileDespawnReasonV1 = Type.Union([
  Type.Literal("hit_target"),
  Type.Literal("hit_block"),
  Type.Literal("out_of_range"),
  Type.Literal("natural_decay"),
]);
export type ProjectileDespawnReasonV1 = Static<typeof ProjectileDespawnReasonV1>;

export const ProjectileDespawnedEventV1 = Type.Object(
  {
    owner: Type.Optional(Type.String({ minLength: 1 })),
    projectile: Type.String({ minLength: 1 }),
    reason: ProjectileDespawnReasonV1,
    distance: Type.Number({ minimum: 0 }),
    qi_evaporated: Type.Number({ minimum: 0 }),
    residual_qi: Type.Number({ minimum: 0 }),
    pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ProjectileDespawnedEventV1 = Static<typeof ProjectileDespawnedEventV1>;

export function validateCarrierStateV1Contract(data: unknown): ValidationResult {
  return validate(CarrierStateV1, data);
}

export function validateCarrierChargedEventV1Contract(data: unknown): ValidationResult {
  return validate(CarrierChargedEventV1, data);
}

export function validateCarrierImpactEventV1Contract(data: unknown): ValidationResult {
  return validate(CarrierImpactEventV1, data);
}

export function validateProjectileDespawnedEventV1Contract(data: unknown): ValidationResult {
  return validate(ProjectileDespawnedEventV1, data);
}
