import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const SenseKindV1 = Type.Union([
  Type.Literal("LivingQi"),
  Type.Literal("AmbientLeyline"),
  Type.Literal("CultivatorRealm"),
  Type.Literal("HeavenlyGaze"),
  Type.Literal("CrisisPremonition"),
]);
export type SenseKindV1 = Static<typeof SenseKindV1>;

export const SenseEntryV1 = Type.Object(
  {
    kind: SenseKindV1,
    x: Type.Number(),
    y: Type.Number(),
    z: Type.Number(),
    intensity: Type.Number({ minimum: 0, maximum: 1 }),
  },
  { additionalProperties: false },
);
export type SenseEntryV1 = Static<typeof SenseEntryV1>;

export const SpiritualSenseTargetsV1 = Type.Object(
  {
    entries: Type.Array(SenseEntryV1, { maxItems: 64 }),
    generation: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SpiritualSenseTargetsV1 = Static<typeof SpiritualSenseTargetsV1>;

export function validateSpiritualSenseTargetsV1Contract(
  data: unknown,
): ValidationResult {
  return validate(SpiritualSenseTargetsV1, data);
}
