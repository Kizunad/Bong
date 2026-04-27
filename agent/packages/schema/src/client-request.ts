/**
 * 客户端 → 服务端请求 schema（plan-cultivation-v1 §P1 剩余 IPC schema）。
 * 覆盖三种交互：
 *   - set_meridian_target：选择下一条要打通的经脉
 *   - breakthrough_request：申请境界突破
 *   - forge_request：请求淬炼某条经脉的 rate 或 capacity
 *
 * 传输层由 Fabric 客户端通过 Minecraft CustomPayload 通道发送，服务端
 * 在 network::mod 中反序列化为对应 Bevy Event。
 */
import { Type, type Static } from "@sinclair/typebox";

import { AlchemyInterventionV1 } from "./alchemy.js";
import { BotanyHarvestModeV1 } from "./botany.js";
import { ForgeAxis } from "./forge-event.js";
import { MeridianId } from "./cultivation.js";
import { ContainerIdV1, EquipSlotV1 } from "./inventory.js";

const JS_SAFE_INTEGER_MAX = Number.MAX_SAFE_INTEGER;
const HOTBAR_SLOT_COUNT = 9;

export const SetMeridianTargetRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("set_meridian_target"),
    meridian: MeridianId,
  },
  { additionalProperties: false },
);
export type SetMeridianTargetRequestV1 = Static<typeof SetMeridianTargetRequestV1>;

export const BreakthroughRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("breakthrough_request"),
  },
  { additionalProperties: false },
);
export type BreakthroughRequestV1 = Static<typeof BreakthroughRequestV1>;

export const ForgeRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_request"),
    meridian: MeridianId,
    axis: ForgeAxis,
  },
  { additionalProperties: false },
);
export type ForgeRequestV1 = Static<typeof ForgeRequestV1>;

export const InsightDecisionRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("insight_decision"),
    trigger_id: Type.String(),
    // null = 拒绝 / 超时（服务端等价处理）；非 null = 选中第 n 个候选。
    choice_idx: Type.Union([Type.Integer({ minimum: 0 }), Type.Null()]),
  },
  { additionalProperties: false },
);
export type InsightDecisionRequestV1 = Static<typeof InsightDecisionRequestV1>;

export const ApplyPillTargetV1 = Type.Union([
  Type.Object(
    {
      kind: Type.Literal("self"),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("meridian"),
      meridian_id: MeridianId,
    },
    { additionalProperties: false },
  ),
]);
export type ApplyPillTargetV1 = Static<typeof ApplyPillTargetV1>;

export const InventoryLocationV1 = Type.Union([
  Type.Object(
    {
      kind: Type.Literal("container"),
      container_id: ContainerIdV1,
      row: Type.Integer({ minimum: 0 }),
      col: Type.Integer({ minimum: 0 }),
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
export type InventoryLocationV1 = Static<typeof InventoryLocationV1>;

export const InventoryMoveIntentRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_move_intent"),
    instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
    from: InventoryLocationV1,
    to: InventoryLocationV1,
  },
  { additionalProperties: false },
);
export type InventoryMoveIntentRequestV1 = Static<typeof InventoryMoveIntentRequestV1>;

export const ApplyPillRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("apply_pill"),
    instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
    target: ApplyPillTargetV1,
  },
  { additionalProperties: false },
);
export type ApplyPillRequestV1 = Static<typeof ApplyPillRequestV1>;

export const DuoSheRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("duo_she_request"),
    target_id: Type.String({ minLength: 1, maxLength: 128 }),
  },
  { additionalProperties: false },
);
export type DuoSheRequestV1 = Static<typeof DuoSheRequestV1>;

export const UseLifeCoreRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("use_life_core"),
    instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type UseLifeCoreRequestV1 = Static<typeof UseLifeCoreRequestV1>;

export const PickupDroppedItemRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("pickup_dropped_item"),
    instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type PickupDroppedItemRequestV1 = Static<typeof PickupDroppedItemRequestV1>;

export const MineralProbeRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("mineral_probe"),
    x: Type.Integer(),
    y: Type.Integer(),
    z: Type.Integer(),
  },
  { additionalProperties: false },
);
export type MineralProbeRequestV1 = Static<typeof MineralProbeRequestV1>;

export const InventoryDiscardItemRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_discard_item"),
    instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
    from: InventoryLocationV1,
  },
  { additionalProperties: false },
);
export type InventoryDiscardItemRequestV1 = Static<typeof InventoryDiscardItemRequestV1>;

export const DropWeaponIntentRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("drop_weapon_intent"),
    instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
    from: InventoryLocationV1,
  },
  { additionalProperties: false },
);
export type DropWeaponIntentRequestV1 = Static<typeof DropWeaponIntentRequestV1>;

export const RepairWeaponIntentRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("repair_weapon_intent"),
    instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
    station_pos: Type.Tuple([Type.Integer(), Type.Integer(), Type.Integer()]),
  },
  { additionalProperties: false },
);
export type RepairWeaponIntentRequestV1 = Static<typeof RepairWeaponIntentRequestV1>;

export const BotanyHarvestRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("botany_harvest_request"),
    session_id: Type.String({ minLength: 1 }),
    mode: BotanyHarvestModeV1,
  },
  { additionalProperties: false },
);
export type BotanyHarvestRequestV1 = Static<typeof BotanyHarvestRequestV1>;

export const CombatReincarnateRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("combat_reincarnate"),
  },
  { additionalProperties: false },
);
export type CombatReincarnateRequestV1 = Static<typeof CombatReincarnateRequestV1>;

export const CombatTerminateRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("combat_terminate"),
  },
  { additionalProperties: false },
);
export type CombatTerminateRequestV1 = Static<typeof CombatTerminateRequestV1>;

export const CombatCreateNewCharacterRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("combat_create_new_character"),
  },
  { additionalProperties: false },
);
export type CombatCreateNewCharacterRequestV1 = Static<typeof CombatCreateNewCharacterRequestV1>;

// ─── 炼丹请求（plan-alchemy-v1 §4） ────────────────────────────────────────

export const AlchemyOpenFurnaceRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_open_furnace"),
    /** 炉的逻辑 ID（BlockEntity 位置 hash 或 Entity ID 字符串化）。 */
    furnace_id: Type.String(),
  },
  { additionalProperties: false },
);
export type AlchemyOpenFurnaceRequestV1 = Static<typeof AlchemyOpenFurnaceRequestV1>;

export const AlchemyFeedSlotRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_feed_slot"),
    /** 槽位 0..3（plan §3.3 四投料槽）。 */
    slot_idx: Type.Integer({ minimum: 0, maximum: 7 }),
    material: Type.String(),
    count: Type.Integer({ minimum: 1 }),
  },
  { additionalProperties: false },
);
export type AlchemyFeedSlotRequestV1 = Static<typeof AlchemyFeedSlotRequestV1>;

export const AlchemyTakeBackRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_take_back"),
    slot_idx: Type.Integer({ minimum: 0, maximum: 7 }),
  },
  { additionalProperties: false },
);
export type AlchemyTakeBackRequestV1 = Static<typeof AlchemyTakeBackRequestV1>;

export const AlchemyIgniteRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_ignite"),
    /** 起炉绑定的配方 — 决定 fire_profile / stages。 */
    recipe_id: Type.String(),
  },
  { additionalProperties: false },
);
export type AlchemyIgniteRequestV1 = Static<typeof AlchemyIgniteRequestV1>;

export const AlchemyInterventionRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_intervention"),
    intervention: AlchemyInterventionV1,
  },
  { additionalProperties: false },
);
export type AlchemyInterventionRequestV1 = Static<typeof AlchemyInterventionRequestV1>;

export const AlchemyTurnPageRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_turn_page"),
    /** -1 / +1 翻页；其他绝对偏移由服务端 mod。 */
    delta: Type.Integer(),
  },
  { additionalProperties: false },
);
export type AlchemyTurnPageRequestV1 = Static<typeof AlchemyTurnPageRequestV1>;

export const AlchemyLearnRecipeRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_learn_recipe"),
    /** 残卷 item 上承载的 recipe_id（client 已从 itemId 提取）。 */
    recipe_id: Type.String(),
  },
  { additionalProperties: false },
);
export type AlchemyLearnRecipeRequestV1 = Static<typeof AlchemyLearnRecipeRequestV1>;

export const AlchemyTakePillRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_take_pill"),
    pill_item_id: Type.String(),
  },
  { additionalProperties: false },
);
export type AlchemyTakePillRequestV1 = Static<typeof AlchemyTakePillRequestV1>;

export const AlchemyFurnacePlaceRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_furnace_place"),
    x: Type.Integer(),
    y: Type.Integer(),
    z: Type.Integer(),
    /** 炉类物品的 inventory instance_id — server 校验并消耗一个。 */
    item_instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type AlchemyFurnacePlaceRequestV1 = Static<typeof AlchemyFurnacePlaceRequestV1>;

export const LearnSkillScrollRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("learn_skill_scroll"),
    instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type LearnSkillScrollRequestV1 = Static<typeof LearnSkillScrollRequestV1>;

export const StartExtractRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("start_extract_request"),
    portal_entity_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type StartExtractRequestV1 = Static<typeof StartExtractRequestV1>;

export const CancelExtractRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("cancel_extract_request"),
  },
  { additionalProperties: false },
);
export type CancelExtractRequestV1 = Static<typeof CancelExtractRequestV1>;

// ─── 炼器（武器）（plan-forge-v1 §4） ────────────────────
export const ForgeStartSessionRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_start_session"),
    station_id: Type.String(),
    blueprint_id: Type.String(),
    materials: Type.Array(
      Type.Tuple([Type.String(), Type.Integer({ minimum: 1 })]),
    ),
  },
  { additionalProperties: false },
);
export type ForgeStartSessionRequestV1 = Static<typeof ForgeStartSessionRequestV1>;

export const ForgeTemperingHitRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_tempering_hit"),
    session_id: Type.Integer({ minimum: 0 }),
    beat: Type.Union([Type.Literal("L"), Type.Literal("H"), Type.Literal("F")]),
    ticks_remaining: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ForgeTemperingHitRequestV1 = Static<typeof ForgeTemperingHitRequestV1>;

export const ForgeInscriptionScrollRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_inscription_scroll"),
    session_id: Type.Integer({ minimum: 0 }),
    inscription_id: Type.String(),
  },
  { additionalProperties: false },
);
export type ForgeInscriptionScrollRequestV1 = Static<typeof ForgeInscriptionScrollRequestV1>;

export const ForgeConsecrationInjectRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_consecration_inject"),
    session_id: Type.Integer({ minimum: 0 }),
    qi_amount: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ForgeConsecrationInjectRequestV1 = Static<typeof ForgeConsecrationInjectRequestV1>;

export const ForgeStepAdvanceRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_step_advance"),
    session_id: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ForgeStepAdvanceRequestV1 = Static<typeof ForgeStepAdvanceRequestV1>;

export const ForgeBlueprintTurnPageRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_blueprint_turn_page"),
    delta: Type.Integer(),
  },
  { additionalProperties: false },
);
export type ForgeBlueprintTurnPageRequestV1 = Static<typeof ForgeBlueprintTurnPageRequestV1>;

export const ForgeLearnBlueprintRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_learn_blueprint"),
    blueprint_id: Type.String(),
  },
  { additionalProperties: false },
);
export type ForgeLearnBlueprintRequestV1 = Static<typeof ForgeLearnBlueprintRequestV1>;

export const ForgeStationPlaceRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_station_place"),
    x: Type.Integer(),
    y: Type.Integer(),
    z: Type.Integer(),
    item_instance_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type ForgeStationPlaceRequestV1 = Static<typeof ForgeStationPlaceRequestV1>;

export const ClientRequestV1 = Type.Union([
  SetMeridianTargetRequestV1,
  BreakthroughRequestV1,
  ForgeRequestV1,
  InsightDecisionRequestV1,
  InventoryMoveIntentRequestV1,
  ApplyPillRequestV1,
  DuoSheRequestV1,
  UseLifeCoreRequestV1,
  PickupDroppedItemRequestV1,
  MineralProbeRequestV1,
  InventoryDiscardItemRequestV1,
  DropWeaponIntentRequestV1,
  RepairWeaponIntentRequestV1,
  BotanyHarvestRequestV1,
  CombatReincarnateRequestV1,
  CombatTerminateRequestV1,
  CombatCreateNewCharacterRequestV1,
  AlchemyOpenFurnaceRequestV1,
  AlchemyFeedSlotRequestV1,
  AlchemyTakeBackRequestV1,
  AlchemyIgniteRequestV1,
  AlchemyInterventionRequestV1,
  AlchemyTurnPageRequestV1,
  AlchemyLearnRecipeRequestV1,
  AlchemyTakePillRequestV1,
  AlchemyFurnacePlaceRequestV1,
  LearnSkillScrollRequestV1,
  StartExtractRequestV1,
  CancelExtractRequestV1,
  ForgeStartSessionRequestV1,
  ForgeTemperingHitRequestV1,
  ForgeInscriptionScrollRequestV1,
  ForgeConsecrationInjectRequestV1,
  ForgeStepAdvanceRequestV1,
  ForgeBlueprintTurnPageRequestV1,
  ForgeLearnBlueprintRequestV1,
  ForgeStationPlaceRequestV1,
]);
export type ClientRequestV1 = Static<typeof ClientRequestV1>;
