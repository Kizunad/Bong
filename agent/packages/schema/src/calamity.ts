import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const CalamityKindV1 = Type.Union([
  Type.Literal("thunder"),
  Type.Literal("poison_miasma"),
  Type.Literal("meridian_seal"),
  Type.Literal("daoxiang_wave"),
  Type.Literal("heavenly_fire"),
  Type.Literal("pressure_invert"),
  Type.Literal("all_wither"),
  Type.Literal("realm_collapse"),
]);
export type CalamityKindV1 = Static<typeof CalamityKindV1>;

export const CalamityIntentV1 = Type.Object(
  {
    v: Type.Literal(1),
    calamity: Type.Union([CalamityKindV1, Type.Null()]),
    target_zone: Type.Optional(Type.Union([Type.String({ minLength: 1 }), Type.Null()])),
    target_player: Type.Optional(Type.Union([Type.String({ minLength: 1 }), Type.Null()])),
    intensity: Type.Number({ minimum: 0, maximum: 1 }),
    reason: Type.String({ minLength: 1, maxLength: 100 }),
  },
  { additionalProperties: false },
);
export type CalamityIntentV1 = Static<typeof CalamityIntentV1>;

export function validateCalamityIntentV1Contract(data: unknown): ValidationResult {
  return validate(CalamityIntentV1, data);
}
