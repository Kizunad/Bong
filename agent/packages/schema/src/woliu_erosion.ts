import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const VoidErosionStageV1 = Type.Union([
  Type.Literal("none"),
  Type.Literal("low_pressure"),
  Type.Literal("void_shadow"),
  Type.Literal("echo_body"),
  Type.Literal("void_eroded"),
]);
export type VoidErosionStageV1 = Static<typeof VoidErosionStageV1>;

export const VoidErosionStateV1 = Type.Object(
  {
    entity: Type.String({ minLength: 1 }),
    stage: VoidErosionStageV1,
    cumulative_erosion: Type.Number({ minimum: 0 }),
    ambient_active: Type.Boolean(),
    contam_mult: Type.Number({ minimum: 0 }),
    efficiency: Type.Number({ minimum: 0, maximum: 1 }),
    server_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type VoidErosionStateV1 = Static<typeof VoidErosionStateV1>;

export const VoidErosionEventV1 = Type.Object(
  {
    entity: Type.String({ minLength: 1 }),
    from_stage: VoidErosionStageV1,
    to_stage: VoidErosionStageV1,
    cumulative_erosion: Type.Number({ minimum: 0 }),
    server_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type VoidErosionEventV1 = Static<typeof VoidErosionEventV1>;

export function validateVoidErosionStateV1Contract(
  data: unknown,
): ValidationResult {
  return validate(VoidErosionStateV1, data);
}

export function validateVoidErosionEventV1Contract(
  data: unknown,
): ValidationResult {
  return validate(VoidErosionEventV1, data);
}
