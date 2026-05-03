import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const ViolationKindV1 = Type.Union([
  Type.Literal("reach_exceeded"),
  Type.Literal("cooldown_bypassed"),
  Type.Literal("qi_invest_exceeded"),
]);
export type ViolationKindV1 = Static<typeof ViolationKindV1>;

export const AntiCheatReportV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("anticheat_report"),
    char_id: Type.String({ minLength: 1 }),
    entity_id: Type.Integer({ minimum: 0 }),
    at_tick: Type.Integer({ minimum: 0 }),
    kind: ViolationKindV1,
    count: Type.Integer({ minimum: 1 }),
    details: Type.String({ minLength: 1 }),
  },
  { additionalProperties: false },
);
export type AntiCheatReportV1 = Static<typeof AntiCheatReportV1>;

export function validateAntiCheatReportV1Contract(
  data: unknown,
): ValidationResult {
  return validate(AntiCheatReportV1, data);
}
