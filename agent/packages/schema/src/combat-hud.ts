import { Type, type Static } from "@sinclair/typebox";

const HOTBAR_SLOT_COUNT = 9;

/** 服务端计算的 HUD 派生状态，供客户端只读展示。 */
export const DerivedAttrFlagsV1 = Type.Object(
  {
    flying: Type.Boolean(),
    phasing: Type.Boolean(),
    tribulation_locked: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type DerivedAttrFlagsV1 = Static<typeof DerivedAttrFlagsV1>;

/**
 * 服务端权威的战斗 HUD 快照。
 *
 * `combat_active` 只由 server CombatState 的战斗窗口产生，不能从 HUD 数值或当前 Screen
 * 推断；social open policy 也必须消费这个字段。
 */
export const CombatHudStateV1 = Type.Object(
  {
    hp_percent: Type.Number({ minimum: 0, maximum: 1 }),
    qi_percent: Type.Number({ minimum: 0, maximum: 1 }),
    stamina_percent: Type.Number({ minimum: 0, maximum: 1 }),
    combat_active: Type.Boolean(),
    derived: DerivedAttrFlagsV1,
  },
  { additionalProperties: false },
);
export type CombatHudStateV1 = Static<typeof CombatHudStateV1>;

export const QuickSlotEntryV1 = Type.Object(
  {
    item_id: Type.String({ minLength: 1 }),
    display_name: Type.String({ minLength: 1 }),
    cast_duration_ms: Type.Integer({ minimum: 0, maximum: 0xffff_ffff }),
    cooldown_ms: Type.Integer({ minimum: 0, maximum: 0xffff_ffff }),
    icon_texture: Type.String(),
  },
  { additionalProperties: false },
);
export type QuickSlotEntryV1 = Static<typeof QuickSlotEntryV1>;

export const QuickSlotConfigV1 = Type.Object(
  {
    slots: Type.Array(Type.Union([QuickSlotEntryV1, Type.Null()]), {
      minItems: HOTBAR_SLOT_COUNT,
      maxItems: HOTBAR_SLOT_COUNT,
    }),
    cooldown_until_ms: Type.Array(Type.Integer({ minimum: 0 }), {
      minItems: HOTBAR_SLOT_COUNT,
      maxItems: HOTBAR_SLOT_COUNT,
    }),
    ack_request_id: Type.Optional(Type.String({ minLength: 1, maxLength: 128 })),
    bind_accepted: Type.Optional(Type.Boolean()),
  },
  { additionalProperties: false },
);
export type QuickSlotConfigV1 = Static<typeof QuickSlotConfigV1>;

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
    proficiency_label: Type.String({ minLength: 1 }),
    active: Type.Boolean(),
    description: Type.String(),
    required_realm: Type.String(),
    required_meridians: Type.Array(TechniqueRequiredMeridianV1),
    qi_cost: Type.Number({ minimum: 0 }),
    stamina_cost: Type.Number({ minimum: 0 }),
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
