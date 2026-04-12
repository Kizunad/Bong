import { Type, type Static } from "@sinclair/typebox";

import { MeridianId } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

export const ForgeAxis = Type.Union([Type.Literal("Rate"), Type.Literal("Capacity")]);
export type ForgeAxis = Static<typeof ForgeAxis>;

export const ForgeEventV1 = Type.Object(
  {
    meridian: MeridianId,
    axis: ForgeAxis,
    from_tier: Type.Integer({ minimum: 0, maximum: 16 }),
    to_tier: Type.Integer({ minimum: 0, maximum: 16 }),
    success: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type ForgeEventV1 = Static<typeof ForgeEventV1>;

export function validateForgeEventV1Contract(data: unknown): ValidationResult {
  return validate(ForgeEventV1, data);
}
