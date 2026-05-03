import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const ZongmenOriginIdV1 = Type.Union([
  Type.Literal(1),
  Type.Literal(2),
  Type.Literal(3),
  Type.Literal(4),
  Type.Literal(5),
  Type.Literal(6),
  Type.Literal(7),
]);
export type ZongmenOriginIdV1 = Static<typeof ZongmenOriginIdV1>;

export const ZongCoreActivationV1 = Type.Object(
  {
    v: Type.Literal(1),
    zone_id: Type.String({ minLength: 1 }),
    core_id: Type.String({ minLength: 1 }),
    origin_id: ZongmenOriginIdV1,
    center_xz: Type.Tuple([Type.Number(), Type.Number()]),
    activated_until_tick: Type.Integer({ minimum: 0 }),
    base_qi: Type.Number({ minimum: 0, maximum: 1 }),
    active_qi: Type.Number({ minimum: 0, maximum: 1 }),
    charge_required: Type.Array(Type.String({ minLength: 1 }), { minItems: 1 }),
    narration_radius_blocks: Type.Integer({ minimum: 1 }),
    anomaly_kind: Type.Literal(5),
  },
  { additionalProperties: false },
);
export type ZongCoreActivationV1 = Static<typeof ZongCoreActivationV1>;

export function validateZongCoreActivationV1Contract(data: unknown): ValidationResult {
  return validate(ZongCoreActivationV1, data);
}
