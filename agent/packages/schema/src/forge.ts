/**
 * 炼器（武器）IPC 共享原子（plan-forge-v1 §4 数据契约）。
 *
 * 服务端 → 客户端的 forge_* 推送（砧/会话/结算/图谱书）走 server-data.ts；
 * 客户端 → 服务端的 forge_* 操作（起炉/击键/铭文/开光/翻页/学图谱/放砧）走 client-request.ts。
 * 本文件提供二者共享的原子类型。
 */
import { Type, type Static } from "@sinclair/typebox";

import { ColorKind } from "./cultivation.js";

/** plan §1.3 四步串行（与服务端 ForgeStep 对齐）。 */
export const ForgeStep = Type.Union(
  [
    Type.Literal("billet"),
    Type.Literal("tempering"),
    Type.Literal("inscription"),
    Type.Literal("consecration"),
    Type.Literal("done"),
  ],
  { description: "plan §1.3 四步进程" },
);
export type ForgeStep = Static<typeof ForgeStep>;

/** plan §2 品阶四阶。 */
export const WeaponTier = Type.Union(
  [
    Type.Literal(1),
    Type.Literal(2),
    Type.Literal(3),
    Type.Literal(4),
  ],
  { description: "plan §2 品阶：1=凡器/2=法器/3=灵器/4=道器" },
);
export type WeaponTier = Static<typeof WeaponTier>;

/** 淬炼击键（J=Light, K=Heavy, L=Fold）。 */
export const TemperBeat = Type.Union(
  [
    Type.Literal("L"),
    Type.Literal("H"),
    Type.Literal("F"),
  ],
  { description: "plan §1.3.2 淬炼击键" },
);
export type TemperBeat = Static<typeof TemperBeat>;

/** plan §1.3 五结果桶。 */
export const ForgeOutcomeBucket = Type.Union(
  [
    Type.Literal("perfect"),
    Type.Literal("good"),
    Type.Literal("flawed"),
    Type.Literal("waste"),
    Type.Literal("explode"),
  ],
  { description: "plan §1.3 结果分桶" },
);
export type ForgeOutcomeBucket = Static<typeof ForgeOutcomeBucket>;

/** 坯料步实时状态。 */
export const ForgeStepBilletState = Type.Object(
  {
    step: Type.Literal("billet"),
    materials_in: Type.Array(
      Type.Tuple([Type.String(), Type.Integer({ minimum: 0 })]),
    ),
    active_carrier: Type.Optional(Type.String()),
    resolved_tier_cap: Type.Integer({ minimum: 1 }),
  },
  { additionalProperties: false },
);

/** 淬炼步实时状态。 */
export const ForgeStepTemperingState = Type.Object(
  {
    step: Type.Literal("tempering"),
    pattern: Type.Array(TemperBeat),
    beat_cursor: Type.Integer({ minimum: 0 }),
    hits: Type.Integer({ minimum: 0 }),
    misses: Type.Integer({ minimum: 0 }),
    deviation: Type.Integer({ minimum: 0 }),
    qi_spent: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);

/** 铭文步实时状态。 */
export const ForgeStepInscriptionState = Type.Object(
  {
    step: Type.Literal("inscription"),
    filled_slots: Type.Integer({ minimum: 0 }),
    max_slots: Type.Integer({ minimum: 0 }),
    failed: Type.Boolean(),
  },
  { additionalProperties: false },
);

/** 开光步实时状态。 */
export const ForgeStepConsecrationState = Type.Object(
  {
    step: Type.Literal("consecration"),
    qi_injected: Type.Number({ minimum: 0 }),
    qi_required: Type.Number({ minimum: 0 }),
    color_imprint: Type.Optional(ColorKind),
  },
  { additionalProperties: false },
);

/** 无步骤状态。 */
export const ForgeStepNoneState = Type.Object(
  {
    step: Type.Literal("none"),
  },
  { additionalProperties: false },
);

/** 各步实时状态（tagged union）。 */
export const ForgeStepState = Type.Union(
  [
    ForgeStepBilletState,
    ForgeStepTemperingState,
    ForgeStepInscriptionState,
    ForgeStepConsecrationState,
    ForgeStepNoneState,
  ],
  { description: "plan §1.3 各步实时状态" },
);
export type ForgeStepState = Static<typeof ForgeStepState>;

/** 单条已学图谱条目。 */
export const ForgeBlueprintEntryV1 = Type.Object(
  {
    id: Type.String(),
    display_name: Type.String(),
    tier_cap: Type.Integer({ minimum: 1, maximum: 4 }),
    step_count: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ForgeBlueprintEntryV1 = Static<typeof ForgeBlueprintEntryV1>;

// ─── server → client payload 结构体（server-data.ts 中组装） ──────────

/** 砧信息快照。 */
export const WeaponForgeStationDataV1 = Type.Object(
  {
    station_id: Type.String(),
    tier: Type.Integer({ minimum: 1, maximum: 4 }),
    integrity: Type.Number({ minimum: 0, maximum: 1 }),
    owner_name: Type.String(),
    has_session: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type WeaponForgeStationDataV1 = Static<typeof WeaponForgeStationDataV1>;

/** 锻造会话实时状态。 */
export const ForgeSessionDataV1 = Type.Object(
  {
    session_id: Type.Integer({ minimum: 0 }),
    blueprint_id: Type.String(),
    blueprint_name: Type.String(),
    active: Type.Boolean(),
    current_step: ForgeStep,
    step_index: Type.Integer({ minimum: 0 }),
    achieved_tier: Type.Integer({ minimum: 0, maximum: 4 }),
    step_state: ForgeStepState,
  },
  { additionalProperties: false },
);
export type ForgeSessionDataV1 = Static<typeof ForgeSessionDataV1>;

/** 锻造结果结算推送。 */
export const ForgeOutcomeDataV1 = Type.Object(
  {
    session_id: Type.Integer({ minimum: 0 }),
    blueprint_id: Type.String(),
    bucket: ForgeOutcomeBucket,
    weapon_item: Type.Optional(Type.String()),
    quality: Type.Number({ minimum: 0, maximum: 1 }),
    color: Type.Optional(ColorKind),
    side_effects: Type.Array(Type.String()),
    achieved_tier: Type.Integer({ minimum: 0, maximum: 4 }),
    flawed_path: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type ForgeOutcomeDataV1 = Static<typeof ForgeOutcomeDataV1>;

/** 已学图谱书快照。 */
export const ForgeBlueprintBookDataV1 = Type.Object(
  {
    learned: Type.Array(ForgeBlueprintEntryV1),
    current_index: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ForgeBlueprintBookDataV1 = Static<typeof ForgeBlueprintBookDataV1>;
