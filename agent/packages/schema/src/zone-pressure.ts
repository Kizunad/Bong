import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const ZonePressureLevelV1 = Type.Union([
  Type.Literal("low"),
  Type.Literal("mid"),
  Type.Literal("high"),
]);
export type ZonePressureLevelV1 = Static<typeof ZonePressureLevelV1>;

export const ZonePressureCrossedV1 = Type.Object(
  {
    v: Type.Literal(1),
    kind: Type.Literal("zone_pressure_crossed"),
    zone: Type.String({ minLength: 1 }),
    level: ZonePressureLevelV1,
    raw_pressure: Type.Number(),
    at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ZonePressureCrossedV1 = Static<typeof ZonePressureCrossedV1>;

export function validateZonePressureCrossedV1Contract(data: unknown): ValidationResult {
  return validate(ZonePressureCrossedV1, data);
}
