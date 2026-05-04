import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const FalseSkinKindV1 = Type.Union([
  Type.Literal("spider_silk"),
  Type.Literal("rotten_wood_armor"),
]);
export type FalseSkinKindV1 = Static<typeof FalseSkinKindV1>;

export const FalseSkinStateV1 = Type.Object(
  {
    target_id: Type.String({ minLength: 1 }),
    kind: Type.Union([FalseSkinKindV1, Type.Null()]),
    layers_remaining: Type.Integer({ minimum: 0, maximum: 3 }),
    contam_capacity_per_layer: Type.Number({ minimum: 0 }),
    absorbed_contam: Type.Number({ minimum: 0 }),
    equipped_at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FalseSkinStateV1 = Static<typeof FalseSkinStateV1>;

export const ShedEventV1 = Type.Object(
  {
    target_id: Type.String({ minLength: 1 }),
    attacker_id: Type.Optional(Type.Union([Type.String({ minLength: 1 }), Type.Null()])),
    kind: FalseSkinKindV1,
    layers_shed: Type.Integer({ minimum: 1, maximum: 3 }),
    layers_remaining: Type.Integer({ minimum: 0, maximum: 3 }),
    contam_absorbed: Type.Number({ minimum: 0 }),
    contam_overflow: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ShedEventV1 = Static<typeof ShedEventV1>;

export function validateFalseSkinStateV1Contract(data: unknown): ValidationResult {
  return validate(FalseSkinStateV1, data);
}

export function validateShedEventV1Contract(data: unknown): ValidationResult {
  return validate(ShedEventV1, data);
}
