import { Type, type Static } from "@sinclair/typebox";

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
]);
export type EquipSlotV1 = Static<typeof EquipSlotV1>;

export const ItemRarityV1 = Type.Union([
  Type.Literal("common"),
  Type.Literal("uncommon"),
  Type.Literal("rare"),
  Type.Literal("epic"),
  Type.Literal("legendary"),
]);
export type ItemRarityV1 = Static<typeof ItemRarityV1>;

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
  InventoryEventStackChangedV1,
  InventoryEventDurabilityChangedV1,
]);
export type InventoryEventV1 = Static<typeof InventoryEventV1>;
