import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const REALM_VISION_VERSION = 1;
export const FLOOR_CLAMP_M = 15;

export const FogShapeV1 = Type.Union([
  Type.Literal("Cylinder"),
  Type.Literal("Sphere"),
]);
export type FogShapeV1 = Static<typeof FogShapeV1>;

export const RealmVisionParamsV1 = Type.Object(
  {
    fog_start: Type.Number({ minimum: 0 }),
    fog_end: Type.Number({ minimum: 0 }),
    fog_color_rgb: Type.Integer({ minimum: 0, maximum: 0xffffff }),
    fog_shape: FogShapeV1,
    vignette_alpha: Type.Number({ minimum: 0, maximum: 1 }),
    tint_color_argb: Type.Integer({ minimum: 0, maximum: 0xffffffff }),
    particle_density: Type.Number({ minimum: 0, maximum: 1 }),
    transition_ticks: Type.Integer({ minimum: 0 }),
    server_view_distance_chunks: Type.Integer({ minimum: 2, maximum: 32 }),
    post_fx_sharpen: Type.Number({ minimum: 0, maximum: 1 }),
  },
  { additionalProperties: false },
);
export type RealmVisionParamsV1 = Static<typeof RealmVisionParamsV1>;

export function validateRealmVisionParamsV1Contract(
  data: unknown,
): ValidationResult {
  return validate(RealmVisionParamsV1, data);
}
