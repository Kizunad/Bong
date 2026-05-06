import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const LingtianZonePressureLevelV1 = Type.Union([
  Type.Literal("low"),
  Type.Literal("mid"),
  Type.Literal("high"),
]);
export type LingtianZonePressureLevelV1 = Static<typeof LingtianZonePressureLevelV1>;

export const LingtianZonePressureV1 = Type.Object(
  {
    v: Type.Literal(1),
    zone: Type.String({ minLength: 1 }),
    level: LingtianZonePressureLevelV1,
    raw_pressure: Type.Number(),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type LingtianZonePressureV1 = Static<typeof LingtianZonePressureV1>;

export function validateLingtianZonePressureV1Contract(data: unknown): ValidationResult {
  return validate(LingtianZonePressureV1, data);
}
