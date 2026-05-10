import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const WoliuSkillIdV1 = Type.Union([
  Type.Literal("hold"),
  Type.Literal("burst"),
  Type.Literal("mouth"),
  Type.Literal("pull"),
  Type.Literal("heart"),
  Type.Literal("vacuum_palm"),
  Type.Literal("vortex_shield"),
  Type.Literal("vacuum_lock"),
  Type.Literal("vortex_resonance"),
  Type.Literal("turbulence_burst"),
]);
export type WoliuSkillIdV1 = Static<typeof WoliuSkillIdV1>;

export const WoliuBackfireLevelV1 = Type.Union([
  Type.Literal("sensation"),
  Type.Literal("micro_tear"),
  Type.Literal("torn"),
  Type.Literal("severed"),
]);
export type WoliuBackfireLevelV1 = Static<typeof WoliuBackfireLevelV1>;

export const WoliuSkillCastV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    skill: WoliuSkillIdV1,
    tick: Type.Integer({ minimum: 0 }),
    lethal_radius: Type.Number({ minimum: 0 }),
    influence_radius: Type.Number({ minimum: 0 }),
    turbulence_radius: Type.Number({ minimum: 0 }),
    absorbed_qi: Type.Number({ minimum: 0 }),
    swirl_qi: Type.Number({ minimum: 0 }),
    animation_id: Type.String({ minLength: 1 }),
    particle_id: Type.String({ minLength: 1 }),
    sound_recipe_id: Type.String({ minLength: 1 }),
    icon_texture: Type.String({ minLength: 1 }),
  },
  { additionalProperties: false },
);
export type WoliuSkillCastV1 = Static<typeof WoliuSkillCastV1>;

export const WoliuBackfireV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    skill: WoliuSkillIdV1,
    level: WoliuBackfireLevelV1,
    cause: Type.String({ minLength: 1 }),
    overflow_qi: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type WoliuBackfireV1 = Static<typeof WoliuBackfireV1>;

export const TurbulenceFieldV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    skill: WoliuSkillIdV1,
    center: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    radius: Type.Number({ minimum: 0 }),
    intensity: Type.Number({ minimum: 0, maximum: 1 }),
    swirl_qi: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type TurbulenceFieldV1 = Static<typeof TurbulenceFieldV1>;

export const WoliuPullDisplaceV1 = Type.Object(
  {
    caster: Type.String({ minLength: 1 }),
    target: Type.String({ minLength: 1 }),
    displacement_blocks: Type.Number({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type WoliuPullDisplaceV1 = Static<typeof WoliuPullDisplaceV1>;

export function validateWoliuSkillCastV1Contract(data: unknown): ValidationResult {
  return validate(WoliuSkillCastV1, data);
}

export function validateWoliuBackfireV1Contract(data: unknown): ValidationResult {
  return validate(WoliuBackfireV1, data);
}

export function validateTurbulenceFieldV1Contract(data: unknown): ValidationResult {
  return validate(TurbulenceFieldV1, data);
}

export function validateWoliuPullDisplaceV1Contract(data: unknown): ValidationResult {
  return validate(WoliuPullDisplaceV1, data);
}
