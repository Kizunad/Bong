import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

/** plan §4 — 修炼侧致死触发。context 透传给战斗 plan 的死亡裁决。 */
export const CultivationDeathCause = Type.Union([
  Type.Literal("BreakthroughBackfire"),
  Type.Literal("TribulationFailure"),
  Type.Literal("MeridianCollapse"),
  Type.Literal("NegativeZoneDrain"),
  Type.Literal("ContaminationOverflow"),
]);
export type CultivationDeathCause = Static<typeof CultivationDeathCause>;

export const CultivationDeathV1 = Type.Object(
  {
    cause: CultivationDeathCause,
    context: Type.Any(),
  },
  { additionalProperties: false },
);
export type CultivationDeathV1 = Static<typeof CultivationDeathV1>;

export function validateCultivationDeathV1Contract(data: unknown): ValidationResult {
  return validate(CultivationDeathV1, data);
}
