import { Type, type Static } from "@sinclair/typebox";

const HOTBAR_SLOT_COUNT = 9;

export const SkillBarItemEntryV1 = Type.Object(
  {
    kind: Type.Literal("item"),
    template_id: Type.String({ minLength: 1 }),
    display_name: Type.String({ minLength: 1 }),
    cast_duration_ms: Type.Integer({ minimum: 0 }),
    cooldown_ms: Type.Integer({ minimum: 0 }),
    icon_texture: Type.String(),
  },
  { additionalProperties: false },
);
export type SkillBarItemEntryV1 = Static<typeof SkillBarItemEntryV1>;

export const SkillBarSkillEntryV1 = Type.Object(
  {
    kind: Type.Literal("skill"),
    skill_id: Type.String({ minLength: 1 }),
    display_name: Type.String({ minLength: 1 }),
    cast_duration_ms: Type.Integer({ minimum: 0 }),
    cooldown_ms: Type.Integer({ minimum: 0 }),
    icon_texture: Type.String(),
  },
  { additionalProperties: false },
);
export type SkillBarSkillEntryV1 = Static<typeof SkillBarSkillEntryV1>;

export const SkillBarEntryV1 = Type.Union([
  SkillBarItemEntryV1,
  SkillBarSkillEntryV1,
]);
export type SkillBarEntryV1 = Static<typeof SkillBarEntryV1>;

export const SkillBarConfigV1 = Type.Object(
  {
    slots: Type.Array(Type.Union([SkillBarEntryV1, Type.Null()]), {
      minItems: HOTBAR_SLOT_COUNT,
      maxItems: HOTBAR_SLOT_COUNT,
    }),
    cooldown_until_ms: Type.Array(Type.Integer({ minimum: 0 }), {
      minItems: HOTBAR_SLOT_COUNT,
      maxItems: HOTBAR_SLOT_COUNT,
    }),
  },
  { additionalProperties: false },
);
export type SkillBarConfigV1 = Static<typeof SkillBarConfigV1>;

export const TechniqueRequiredMeridianV1 = Type.Object(
  {
    channel: Type.String({ minLength: 1 }),
    min_health: Type.Number({ minimum: 0, maximum: 1 }),
  },
  { additionalProperties: false },
);
export type TechniqueRequiredMeridianV1 = Static<typeof TechniqueRequiredMeridianV1>;

export const TechniqueEntryV1 = Type.Object(
  {
    id: Type.String({ minLength: 1 }),
    display_name: Type.String({ minLength: 1 }),
    grade: Type.String({ minLength: 1 }),
    proficiency: Type.Number({ minimum: 0, maximum: 1 }),
    active: Type.Boolean(),
    description: Type.String(),
    required_realm: Type.String(),
    required_meridians: Type.Array(TechniqueRequiredMeridianV1),
    qi_cost: Type.Integer({ minimum: 0 }),
    cast_ticks: Type.Integer({ minimum: 0 }),
    cooldown_ticks: Type.Integer({ minimum: 0 }),
    range: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type TechniqueEntryV1 = Static<typeof TechniqueEntryV1>;

export const TechniquesSnapshotV1 = Type.Object(
  {
    entries: Type.Array(TechniqueEntryV1),
  },
  { additionalProperties: false },
);
export type TechniquesSnapshotV1 = Static<typeof TechniquesSnapshotV1>;
