import { Type, type Static } from "@sinclair/typebox";

import { ColorKind } from "./cultivation.js";

const JS_SAFE_INTEGER_MAX = Number.MAX_SAFE_INTEGER;
const HOTBAR_SLOT_COUNT = 9;

const SafeIntegerV1 = Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX });
const RevisionV1 = Type.Integer({ minimum: 0 });
const GridCoordinateV1 = Type.Integer({ minimum: 0 });
const GridSpanV1 = Type.Integer({ minimum: 1, maximum: 4 });
const ContainerExtentV1 = Type.Integer({ minimum: 1, maximum: 16 });

export const ContainerIdV1 = Type.Union([
  Type.Literal("main_pack"),
  Type.Literal("small_pouch"),
  Type.Literal("front_satchel"),
]);
export type ContainerIdV1 = Static<typeof ContainerIdV1>;

export const EquipSlotV1 = Type.Union([
  Type.Literal("head"),
  Type.Literal("chest"),
  Type.Literal("legs"),
  Type.Literal("feet"),
  Type.Literal("main_hand"),
  Type.Literal("off_hand"),
  Type.Literal("two_hand"),
  Type.Literal("treasure_belt_0"),
  Type.Literal("treasure_belt_1"),
  Type.Literal("treasure_belt_2"),
  Type.Literal("treasure_belt_3"),
]);
export type EquipSlotV1 = Static<typeof EquipSlotV1>;

export const ItemRarityV1 = Type.Union([
  Type.Literal("common"),
  Type.Literal("uncommon"),
  Type.Literal("rare"),
  Type.Literal("epic"),
  Type.Literal("legendary"),
  // plan-tsy-loot-v1 §1.4 — TSY 上古遗物。spirit_quality 恒为 0，
  // durability 字段语义切换为"剩余使用次数"（1/3/5）。
  Type.Literal("ancient"),
]);
export type ItemRarityV1 = Static<typeof ItemRarityV1>;

// plan-shelflife-v1 §0.4 / §2.1 — 物品保质期 NBT 镜像（与 server 端
// crate::shelflife::Freshness 对齐）。
export const DecayTrackV1 = Type.Union([
  Type.Literal("Decay"),
  Type.Literal("Spoil"),
  Type.Literal("Age"),
]);
export type DecayTrackV1 = Static<typeof DecayTrackV1>;

export const FreshnessV1 = Type.Object(
  {
    created_at_tick: SafeIntegerV1,
    // initial_qi 必须非负 — shelflife compute_* headroom / exp-decay 公式假设 >= 0
    initial_qi: Type.Number({ minimum: 0 }),
    track: DecayTrackV1,
    profile: Type.String({ minLength: 1, maxLength: 128 }),
    frozen_accumulated: Type.Optional(SafeIntegerV1),
    frozen_since_tick: Type.Optional(Type.Union([SafeIntegerV1, Type.Null()])),
  },
  { additionalProperties: false },
);
export type FreshnessV1 = Static<typeof FreshnessV1>;

// plan-shelflife-v1 M3a — TrackState 路径机态（与 server crate::shelflife::TrackState 对齐）。
// 7 档 PascalCase（与 DecayTrack 一致）— client M3b 由此 + current_qi 比率衍生 5 档显示位。
export const TrackStateV1 = Type.Union([
  Type.Literal("Fresh"),
  Type.Literal("Declining"),
  Type.Literal("Dead"),
  Type.Literal("Spoiled"),
  Type.Literal("Peaking"),
  Type.Literal("PastPeak"),
  Type.Literal("AgePostPeakSpoiled"),
]);
export type TrackStateV1 = Static<typeof TrackStateV1>;

// plan-shelflife-v1 M3a — 衍生 freshness 数据（snapshot emit 时由 server 预算）。
// client 不需内置 compute_* 逻辑 + DecayProfileRegistry。
export const FreshnessDerivedV1 = Type.Object(
  {
    // current_qi 非负 — compute_current_qi 在所有路径保证（Decay floor_qi ≥ 0 / Spoil max(0) / Age .max(0.0)）
    current_qi: Type.Number({ minimum: 0 }),
    track_state: TrackStateV1,
  },
  { additionalProperties: false },
);
export type FreshnessDerivedV1 = Static<typeof FreshnessDerivedV1>;

export const AlchemySideEffectV1 = Type.Object(
  {
    tag: Type.String({ minLength: 1, maxLength: 128 }),
    duration_s: Type.Optional(Type.Integer({ minimum: 0 })),
    weight: Type.Optional(Type.Integer({ minimum: 0 })),
    perm: Type.Optional(Type.Boolean()),
    color: Type.Optional(ColorKind),
    amount: Type.Optional(Type.Number()),
  },
  { additionalProperties: false },
);
export type AlchemySideEffectV1 = Static<typeof AlchemySideEffectV1>;

export const RecipeFragmentV1 = Type.Object(
  {
    recipe_id: Type.String({ minLength: 1, maxLength: 128 }),
    known_stages: Type.Array(Type.Integer({ minimum: 0, maximum: 255 })),
    max_quality_tier: Type.Integer({ minimum: 1, maximum: 3 }),
  },
  { additionalProperties: false },
);
export type RecipeFragmentV1 = Static<typeof RecipeFragmentV1>;

export const RecipeHintV1 = Type.Object(
  {
    source_pill: Type.String({ minLength: 1, maxLength: 128 }),
    recipe_id: Type.Optional(Type.Union([Type.String({ minLength: 1, maxLength: 128 }), Type.Null()])),
    accuracy: Type.Number({ minimum: 0, maximum: 1 }),
    ingredients: Type.Array(Type.String({ minLength: 1, maxLength: 128 }), { maxItems: 3 }),
  },
  { additionalProperties: false },
);
export type RecipeHintV1 = Static<typeof RecipeHintV1>;

export const AlchemyItemDataV1 = Type.Union([
  Type.Object(
    {
      kind: Type.Literal("pill"),
      recipe_id: Type.String({ minLength: 1, maxLength: 128 }),
      quality_tier: Type.Integer({ minimum: 1, maximum: 5 }),
      effect_multiplier: Type.Number({ minimum: 0 }),
      consecrated: Type.Boolean(),
      side_effect: Type.Optional(AlchemySideEffectV1),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("recipe_fragment"),
      fragment: RecipeFragmentV1,
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("recipe_hint"),
      hint: RecipeHintV1,
    },
    { additionalProperties: false },
  ),
]);
export type AlchemyItemDataV1 = Static<typeof AlchemyItemDataV1>;

export const InventoryItemViewV1 = Type.Object(
  {
    instance_id: SafeIntegerV1,
    item_id: Type.String({ minLength: 1, maxLength: 128 }),
    display_name: Type.String({ minLength: 1, maxLength: 256 }),
    grid_width: GridSpanV1,
    grid_height: GridSpanV1,
    weight: Type.Number({ minimum: 0 }),
    rarity: ItemRarityV1,
    description: Type.String({ maxLength: 4096 }),
    stack_count: Type.Integer({ minimum: 1 }),
    spirit_quality: Type.Number({ minimum: 0, maximum: 1 }),
    durability: Type.Number({ minimum: 0, maximum: 1 }),
    // 物品保质期 NBT；缺省视作"无时间敏感"（凡俗工具 / 瑶器等）。
    freshness: Type.Optional(FreshnessV1),
    // M3a 衍生数据；None = freshness 缺失 / profile 未在 registry / 无法衍生。
    freshness_current: Type.Optional(FreshnessDerivedV1),
    // plan-mineral-v1 §2.2 — mineral_id NBT；从矿脉挖出的物品挂正典 mineral_id
    // 字符串（如 "fan_tie" / "ling_shi_zhong"），非矿物来源 item 留 undefined。
    // alchemy / forge 配方校验 inventory.material 时即按此字段比对。
    mineral_id: Type.Optional(Type.String({ minLength: 1, maxLength: 64 })),
    // plan-tsy-loot-v1 §1.3 — Ancient rarity 物品的"剩余使用次数"。
    // tier 1/3/5 → charges 1/3/5；每次使用 -= 1，归零销毁。
    // 非 ancient 物品恒为 undefined。durability 字段保持 0..=1 不被破坏。
    charges: Type.Optional(Type.Integer({ minimum: 0, maximum: 5 })),
    // plan-forge-leftovers-v1 §2.2 — forge 产物运行时元数据；缺省表示非 forge 产物。
    forge_quality: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    forge_color: Type.Optional(ColorKind),
    forge_side_effects: Type.Optional(Type.Array(Type.String({ minLength: 1, maxLength: 128 }))),
    forge_achieved_tier: Type.Optional(Type.Integer({ minimum: 1, maximum: 4 })),
    // plan-alchemy-v2 — 丹药品阶 / 残卷 / 丹心线索运行时元数据。
    alchemy: Type.Optional(AlchemyItemDataV1),
  },
  { additionalProperties: false },
);
export type InventoryItemViewV1 = Static<typeof InventoryItemViewV1>;

const NullableInventoryItemViewV1 = Type.Union([InventoryItemViewV1, Type.Null()]);

const InventoryWeightV1 = Type.Object(
  {
    current: Type.Number({ minimum: 0 }),
    max: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);

const EquippedInventorySnapshotV1 = Type.Object(
  {
    head: NullableInventoryItemViewV1,
    chest: NullableInventoryItemViewV1,
    legs: NullableInventoryItemViewV1,
    feet: NullableInventoryItemViewV1,
    main_hand: NullableInventoryItemViewV1,
    off_hand: NullableInventoryItemViewV1,
    two_hand: NullableInventoryItemViewV1,
    treasure_belt_0: NullableInventoryItemViewV1,
    treasure_belt_1: NullableInventoryItemViewV1,
    treasure_belt_2: NullableInventoryItemViewV1,
    treasure_belt_3: NullableInventoryItemViewV1,
  },
  { additionalProperties: false },
);

const InventoryLocationV1 = Type.Union([
  Type.Object(
    {
      kind: Type.Literal("container"),
      container_id: ContainerIdV1,
      row: GridCoordinateV1,
      col: GridCoordinateV1,
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("equip"),
      slot: EquipSlotV1,
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("hotbar"),
      index: Type.Integer({ minimum: 0, maximum: HOTBAR_SLOT_COUNT - 1 }),
    },
    { additionalProperties: false },
  ),
]);

export const PlacedInventoryItemV1 = Type.Object(
  {
    container_id: ContainerIdV1,
    row: GridCoordinateV1,
    col: GridCoordinateV1,
    item: InventoryItemViewV1,
  },
  { additionalProperties: false },
);
export type PlacedInventoryItemV1 = Static<typeof PlacedInventoryItemV1>;

export const ContainerSnapshotV1 = Type.Object(
  {
    id: ContainerIdV1,
    name: Type.String({ minLength: 1, maxLength: 64 }),
    rows: ContainerExtentV1,
    cols: ContainerExtentV1,
  },
  { additionalProperties: false },
);
export type ContainerSnapshotV1 = Static<typeof ContainerSnapshotV1>;

export const InventorySnapshotV1 = Type.Object(
  {
    revision: RevisionV1,
    containers: Type.Array(ContainerSnapshotV1, { minItems: 3, maxItems: 3 }),
    placed_items: Type.Array(PlacedInventoryItemV1),
    equipped: EquippedInventorySnapshotV1,
    hotbar: Type.Array(NullableInventoryItemViewV1, {
      minItems: HOTBAR_SLOT_COUNT,
      maxItems: HOTBAR_SLOT_COUNT,
    }),
    bone_coins: SafeIntegerV1,
    weight: InventoryWeightV1,
    realm: Type.String({ minLength: 1, maxLength: 64 }),
    qi_current: Type.Number({ minimum: 0 }),
    qi_max: Type.Number({ minimum: 0 }),
    body_level: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type InventorySnapshotV1 = Static<typeof InventorySnapshotV1>;

export const InventoryEventMovedV1 = Type.Object(
  {
    kind: Type.Literal("moved"),
    revision: RevisionV1,
    instance_id: SafeIntegerV1,
    from: InventoryLocationV1,
    to: InventoryLocationV1,
  },
  { additionalProperties: false },
);

import { Vec3 } from "./world-state.js";

export const InventoryEventDroppedV1 = Type.Object(
  {
    kind: Type.Literal("dropped"),
    revision: RevisionV1,
    instance_id: SafeIntegerV1,
    from: InventoryLocationV1,
    world_pos: Vec3,
    item: InventoryItemViewV1,
  },
  { additionalProperties: false },
);

export const InventoryEventStackChangedV1 = Type.Object(
  {
    kind: Type.Literal("stack_changed"),
    revision: RevisionV1,
    instance_id: SafeIntegerV1,
    stack_count: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);

export const InventoryEventDurabilityChangedV1 = Type.Object(
  {
    kind: Type.Literal("durability_changed"),
    revision: RevisionV1,
    instance_id: SafeIntegerV1,
    durability: Type.Number({ minimum: 0, maximum: 1 }),
  },
  { additionalProperties: false },
);

export const InventoryEventV1 = Type.Union([
  InventoryEventMovedV1,
  InventoryEventDroppedV1,
  InventoryEventStackChangedV1,
  InventoryEventDurabilityChangedV1,
]);
export type InventoryEventV1 = Static<typeof InventoryEventV1>;
