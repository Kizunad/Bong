import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

const Vec3 = Type.Tuple([Type.Number(), Type.Number(), Type.Number()]);
const RgbChannel = Type.Integer({ minimum: 0, maximum: 255 });
const Rgb = Type.Tuple([RgbChannel, RgbChannel, RgbChannel]);

export const EnvironmentEffectV1 = Type.Union([
  Type.Object(
    {
      kind: Type.Literal("tornado_column"),
      center: Vec3,
      radius: Type.Number({ minimum: 0 }),
      height: Type.Number({ minimum: 0 }),
      particle_density: Type.Number({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("lightning_pillar"),
      center: Vec3,
      radius: Type.Number({ minimum: 0 }),
      strike_rate_per_min: Type.Number({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("ash_fall"),
      aabb_min: Vec3,
      aabb_max: Vec3,
      density: Type.Number({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("fog_veil"),
      aabb_min: Vec3,
      aabb_max: Vec3,
      tint_rgb: Rgb,
      density: Type.Number({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("dust_devil"),
      center: Vec3,
      radius: Type.Number({ minimum: 0 }),
      height: Type.Number({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("ember_drift"),
      aabb_min: Vec3,
      aabb_max: Vec3,
      density: Type.Number({ minimum: 0 }),
      glow: Type.Number({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("heat_haze"),
      aabb_min: Vec3,
      aabb_max: Vec3,
      distortion_strength: Type.Number({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("snow_drift"),
      aabb_min: Vec3,
      aabb_max: Vec3,
      density: Type.Number({ minimum: 0 }),
      wind_dir: Vec3,
    },
    { additionalProperties: false },
  ),
]);
export type EnvironmentEffectV1 = Static<typeof EnvironmentEffectV1>;

export const ZoneEnvironmentStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    dimension: Type.String({ minLength: 1 }),
    zone_id: Type.String({ minLength: 1 }),
    effects: Type.Array(EnvironmentEffectV1),
    generation: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ZoneEnvironmentStateV1 = Static<typeof ZoneEnvironmentStateV1>;

export function validateEnvironmentEffectV1Contract(
  data: unknown,
): ValidationResult {
  return validate(EnvironmentEffectV1, data);
}

export function validateZoneEnvironmentStateV1Contract(
  data: unknown,
): ValidationResult {
  return validate(ZoneEnvironmentStateV1, data);
}
