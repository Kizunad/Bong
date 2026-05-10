import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const TuikeV2SkillIdV1 = Type.Union([
  Type.Literal("don"),
  Type.Literal("shed"),
  Type.Literal("transfer_taint"),
]);
export type TuikeV2SkillIdV1 = Static<typeof TuikeV2SkillIdV1>;

export const FalseSkinTierV1 = Type.Union([
  Type.Literal("fan"),
  Type.Literal("light"),
  Type.Literal("mid"),
  Type.Literal("heavy"),
  Type.Literal("ancient"),
]);
export type FalseSkinTierV1 = Static<typeof FalseSkinTierV1>;

export const TuikeV2SkillEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("tuike_v2_skill_event"),
    caster_id: Type.String({ minLength: 1 }),
    skill_id: TuikeV2SkillIdV1,
    tier: FalseSkinTierV1,
    layers_after: Type.Integer({ minimum: 0, maximum: 3 }),
    contam_moved_percent: Type.Number({ minimum: 0 }),
    permanent_absorbed: Type.Number({ minimum: 0 }),
    qi_cost: Type.Number({ minimum: 0 }),
    damage_absorbed: Type.Optional(Type.Number({ minimum: 0 })),
    damage_overflow: Type.Optional(Type.Number({ minimum: 0 })),
    contam_load: Type.Optional(Type.Number({ minimum: 0, maximum: 100 })),
    active_shed: Type.Optional(Type.Boolean()),
    tick: Type.Integer({ minimum: 0 }),
    animation_id: Type.String({ minLength: 1 }),
    particle_id: Type.String({ minLength: 1 }),
    sound_recipe_id: Type.String({ minLength: 1 }),
    icon_texture: Type.String({ minLength: 1 }),
  },
  { additionalProperties: false },
);
export type TuikeV2SkillEventV1 = Static<typeof TuikeV2SkillEventV1>;

export const FalseSkinLayerStateV1 = Type.Object(
  {
    tier: FalseSkinTierV1,
    spirit_quality: Type.Number({ minimum: 0 }),
    damage_capacity: Type.Number({ minimum: 0 }),
    contam_load: Type.Number({ minimum: 0, maximum: 100 }),
    permanent_taint_load: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FalseSkinLayerStateV1 = Static<typeof FalseSkinLayerStateV1>;

export const FalseSkinStackStateV1 = Type.Object(
  {
    owner: Type.String({ minLength: 1 }),
    layers: Type.Array(FalseSkinLayerStateV1, { maxItems: 3 }),
    naked_until_tick: Type.Integer({ minimum: 0 }),
    transfer_permanent_cooldown_until_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type FalseSkinStackStateV1 = Static<typeof FalseSkinStackStateV1>;

export function validateTuikeV2SkillEventV1Contract(data: unknown): ValidationResult {
  return validate(TuikeV2SkillEventV1, data);
}

export function validateFalseSkinStackStateV1Contract(data: unknown): ValidationResult {
  return validate(FalseSkinStackStateV1, data);
}
