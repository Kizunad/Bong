import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const RatPhaseV1 = Type.Union([
  Type.Literal("solitary"),
  Type.Object(
    {
      transitioning: Type.Object(
        {
          progress: Type.Integer({ minimum: 0 }),
        },
        { additionalProperties: false },
      ),
    },
    { additionalProperties: false },
  ),
  Type.Literal("gregarious"),
]);
export type RatPhaseV1 = Static<typeof RatPhaseV1>;

export const RatPhaseChangeEventV1 = Type.Object(
  {
    chunk: Type.Tuple([Type.Integer(), Type.Integer()]),
    zone: Type.String(),
    group_id: Type.Integer({ minimum: 0 }),
    from: RatPhaseV1,
    to: RatPhaseV1,
    rat_count: Type.Integer({ minimum: 0 }),
    local_qi: Type.Number({ minimum: 0, maximum: 1 }),
    qi_gradient: Type.Number({ minimum: 0, maximum: 1 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type RatPhaseChangeEventV1 = Static<typeof RatPhaseChangeEventV1>;

export function validateRatPhaseChangeEventV1Contract(data: unknown): ValidationResult {
  return validate(RatPhaseChangeEventV1, data);
}
