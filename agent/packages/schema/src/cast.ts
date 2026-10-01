import { Type, type Static, type TSchema } from "@sinclair/typebox";
import { Value } from "@sinclair/typebox/value";

/**
 * R9 P1 的 TypeBox canonical cast contract。
 *
 * 这里只声明 shape 与 validation semantics。生成的 JSON Schema 是 mirror；本文件没有
 * 被 server/client production router 引用，待 RF-31 的 Wave 2 atomic activation 再接线。
 */

// TypeBox Value.Check 不默认注册 format registry，因此用 canonical UUID pattern 让
// malformed UUID 在 schema 层也 fail-closed，而不是只在某个 bridge 自定义校验。
const UUID = Type.String({
  pattern: "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$",
});
const UINT64_PATTERN = "^[1-9][0-9]*$";

/** TypeBox 能表达非零十进制形状；u64 上界由 `isCanonicalUint64String` 补齐。 */
export const CanonicalUint64String = Type.String({
  pattern: UINT64_PATTERN,
  maxLength: 20,
});
export type CanonicalUint64String = Static<typeof CanonicalUint64String>;

export function isCanonicalUint64String(value: unknown): value is CanonicalUint64String {
  if (typeof value !== "string" || !/^[1-9][0-9]*$/.test(value)) return false;
  try {
    const parsed = BigInt(value);
    return parsed >= 1n && parsed <= 18_446_744_073_709_551_615n;
  } catch {
    return false;
  }
}

export const CastSourceV1 = Type.Union([
  Type.Literal("QUICK_SLOT"),
  Type.Literal("SKILL_BAR"),
  Type.Literal("DEDICATED"),
]);
export type CastSourceV1 = Static<typeof CastSourceV1>;

export const CastPhaseV1 = Type.Union([
  Type.Literal("IDLE"),
  Type.Literal("CASTING"),
  Type.Literal("COMPLETE"),
  Type.Literal("INTERRUPT"),
]);
export type CastPhaseV1 = Static<typeof CastPhaseV1>;

export const CastOutcomeV1 = Type.Union([
  Type.Literal("NONE"),
  Type.Literal("COMPLETED"),
  Type.Literal("INTERRUPT_MOVEMENT"),
  Type.Literal("INTERRUPT_CONTAM"),
  Type.Literal("INTERRUPT_CONTROL"),
  Type.Literal("USER_CANCEL"),
  Type.Literal("DEATH"),
  Type.Literal("MERIDIAN_GATED"),
  Type.Literal("REJECT_QI_INSUFFICIENT"),
  Type.Literal("REJECT_ON_COOLDOWN"),
  Type.Literal("REJECT_INVALID_TARGET"),
  Type.Literal("REJECT_IN_RECOVERY"),
  Type.Literal("REJECT_REALM_TOO_LOW"),
  Type.Literal("REJECT_NO_WEAPON"),
  Type.Literal("REJECT_TECHNIQUE_INACTIVE"),
  Type.Literal("REJECT_RACE_MISMATCH"),
  Type.Literal("REJECT_SKILL_CONFIG_INVALID"),
]);
export type CastOutcomeV1 = Static<typeof CastOutcomeV1>;

export const CastIdentityV1 = Type.Object(
  {
    session_id: UUID,
    session_generation: CanonicalUint64String,
    cast_instance_id: CanonicalUint64String,
  },
  { additionalProperties: false },
);
export type CastIdentityV1 = Static<typeof CastIdentityV1>;

export const CastCasterPlayerV1 = Type.Object(
  { player_uuid: UUID },
  { additionalProperties: false },
);
export const CastCasterNpcV1 = Type.Object(
  { npc_uuid: UUID },
  { additionalProperties: false },
);
export const CastCasterRefV1 = Type.Union([CastCasterPlayerV1, CastCasterNpcV1]);
export type CastCasterRefV1 = Static<typeof CastCasterRefV1>;

export const CastTargetEntityV1 = Type.Object(
  { entity_uuid: UUID },
  { additionalProperties: false },
);
export const CastTargetBlockV1 = Type.Object(
  {
    dimension_id: Type.String({ minLength: 1 }),
    x: Type.Integer(),
    y: Type.Integer(),
    z: Type.Integer(),
  },
  { additionalProperties: false },
);
export const CastTargetRefV1 = Type.Union([CastTargetEntityV1, CastTargetBlockV1]);
export type CastTargetRefV1 = Static<typeof CastTargetRefV1>;

const CastSessionBeginCommon = {
    v: Type.Literal(1),
    caster: CastCasterRefV1,
    target_entity_id: Type.Integer({ minimum: -2_147_483_648, maximum: 2_147_483_647 }),
  session_id: UUID,
  session_generation: CanonicalUint64String,
};

function sessionBeginArm(
  exhausted: boolean,
  active: boolean,
  type?: "cast_session_begin",
) {
  const properties = {
    ...CastSessionBeginCommon,
    allocator_exhausted: Type.Literal(exhausted),
    ...(active
      ? {
          active_cast_instance_id: CanonicalUint64String,
          minimum_cast_instance_id: CanonicalUint64String,
        }
      : {}),
    ...(type === undefined ? {} : { type: Type.Literal(type) }),
  };
  return Type.Object(properties, { additionalProperties: false });
}

const CastSessionBeginOpen = sessionBeginArm(false, false);
const CastSessionBeginOpenActive = sessionBeginArm(false, true);
const CastSessionBeginExhausted = sessionBeginArm(true, false);
const CastSessionBeginExhaustedActive = sessionBeginArm(true, true);

/** P-04/P-05：四种合法 BEGIN wire shape，active 与 floor 必须成对出现。 */
export const CastSessionBeginV1 = Type.Union([
  CastSessionBeginOpen,
  CastSessionBeginOpenActive,
  CastSessionBeginExhausted,
  CastSessionBeginExhaustedActive,
]);
export type CastSessionBeginV1 = Static<typeof CastSessionBeginV1>;

/** P-05 的跨字段约束：schema union 约束字段组合，此 helper 再核对 active 与 floor 相等。 */
export function isCastSessionBeginV1(value: unknown): value is CastSessionBeginV1 {
  if (!Value.Check(CastSessionBeginV1, value)) return false;
  const candidate = value as Record<string, unknown>;
  if (candidate.active_cast_instance_id === undefined) {
    return candidate.minimum_cast_instance_id === undefined;
  }
  return candidate.active_cast_instance_id === candidate.minimum_cast_instance_id;
}

export const CastQuickSlotV1 = Type.Object(
  { kind: Type.Literal("quick_slot"), index: Type.Integer({ minimum: 0, maximum: 8 }) },
  { additionalProperties: false },
);
export const CastSkillBarSlotV1 = Type.Object(
  { kind: Type.Literal("skill_bar") },
  { additionalProperties: false },
);
export const CastDedicatedSlotV1 = Type.Object(
  { kind: Type.Literal("dedicated") },
  { additionalProperties: false },
);
export const CastSlotV1 = Type.Union([
  CastQuickSlotV1,
  CastSkillBarSlotV1,
  CastDedicatedSlotV1,
]);
export type CastSlotV1 = Static<typeof CastSlotV1>;

const CastInterruptOutcomeV1 = Type.Union([
  Type.Literal("INTERRUPT_MOVEMENT"),
  Type.Literal("INTERRUPT_CONTAM"),
  Type.Literal("INTERRUPT_CONTROL"),
  Type.Literal("USER_CANCEL"),
  Type.Literal("DEATH"),
]);
const CastRejectOutcomeV1 = Type.Union([
  Type.Literal("MERIDIAN_GATED"),
  Type.Literal("REJECT_QI_INSUFFICIENT"),
  Type.Literal("REJECT_ON_COOLDOWN"),
  Type.Literal("REJECT_INVALID_TARGET"),
  Type.Literal("REJECT_IN_RECOVERY"),
  Type.Literal("REJECT_REALM_TOO_LOW"),
  Type.Literal("REJECT_NO_WEAPON"),
  Type.Literal("REJECT_TECHNIQUE_INACTIVE"),
  Type.Literal("REJECT_RACE_MISMATCH"),
  Type.Literal("REJECT_SKILL_CONFIG_INVALID"),
]);

const CastSyncCommon = {
  v: Type.Literal(1),
  identity: CastIdentityV1,
  target: Type.Optional(CastTargetRefV1),
  duration_ms: Type.Integer({ minimum: 0 }),
  started_at_ms: Type.Integer({ minimum: 0 }),
};

type CastPhaseLiteral = "CASTING" | "COMPLETE" | "INTERRUPT";

function acceptedCastSyncArm(
  source: "QUICK_SLOT" | "SKILL_BAR" | "DEDICATED",
  npc: boolean,
  phase: CastPhaseLiteral,
  skillId: TSchema,
  slot: TSchema,
  outcome: TSchema,
  type?: "cast_sync",
) {
  return Type.Object(
    {
      ...CastSyncCommon,
      source: Type.Literal(source),
      npc: Type.Literal(npc),
      skill_id: skillId,
      av_binding_key: Type.String({ minLength: 1 }),
      phase: Type.Literal(phase),
      slot,
      outcome,
      ...(type === undefined ? {} : { type: Type.Literal(type) }),
    },
    { additionalProperties: false },
  );
}

const acceptedPhases: CastPhaseLiteral[] = ["CASTING", "COMPLETE", "INTERRUPT"];
const acceptedOutcomeFor = (phase: CastPhaseLiteral): TSchema =>
  phase === "CASTING"
    ? Type.Literal("NONE")
    : phase === "COMPLETE"
      ? Type.Literal("COMPLETED")
      : CastInterruptOutcomeV1;

function acceptedCastSyncArms(type?: "cast_sync") {
  return acceptedPhases.flatMap((phase) => [
    acceptedCastSyncArm(
      "QUICK_SLOT",
      false,
      phase,
      Type.Null(),
      CastQuickSlotV1,
      acceptedOutcomeFor(phase),
      type,
    ),
    acceptedCastSyncArm(
      "SKILL_BAR",
      false,
      phase,
      Type.String({ minLength: 1 }),
      CastSkillBarSlotV1,
      acceptedOutcomeFor(phase),
      type,
    ),
    acceptedCastSyncArm(
      "DEDICATED",
      false,
      phase,
      Type.String({ minLength: 1 }),
      CastDedicatedSlotV1,
      acceptedOutcomeFor(phase),
      type,
    ),
    acceptedCastSyncArm(
      "DEDICATED",
      true,
      phase,
      Type.String({ minLength: 1 }),
      CastDedicatedSlotV1,
      acceptedOutcomeFor(phase),
      type,
    ),
  ]);
}

const acceptedArms = acceptedCastSyncArms();

const rejectBindingPattern =
  "^cast\\.reject\\/(meridian_gated|reject_qi_insufficient|reject_on_cooldown|reject_invalid_target|reject_in_recovery|reject_realm_too_low|reject_no_weapon|reject_technique_inactive|reject_race_mismatch|reject_skill_config_invalid)$";

function rejectCastSyncArm(type?: "cast_sync") {
  return Type.Object(
    {
      ...CastSyncCommon,
      source: Type.Literal("DEDICATED"),
      npc: Type.Literal(false),
      skill_id: Type.Null(),
      av_binding_key: Type.String({ pattern: rejectBindingPattern }),
      phase: Type.Literal("IDLE"),
      slot: CastDedicatedSlotV1,
      outcome: CastRejectOutcomeV1,
      ...(type === undefined ? {} : { type: Type.Literal(type) }),
    },
    { additionalProperties: false },
  );
}

const rejectArm = rejectCastSyncArm();

/** P-06/P-08：accepted casts 与 protocol-owned reject envelope 的严格联合。 */
export const CastSyncV1 = Type.Union([rejectArm, ...acceptedArms]);
export type CastSyncV1 = Static<typeof CastSyncV1>;

export const CastPlayAnimV1 = Type.Object(
  {
    v: Type.Literal(1),
    identity: CastIdentityV1,
    av_binding_key: Type.String({ minLength: 1 }),
    animation_id: Type.String({ minLength: 1 }),
  },
  { additionalProperties: false },
);
export type CastPlayAnimV1 = Static<typeof CastPlayAnimV1>;

export const CastStopAnimV1 = Type.Object(
  {
    v: Type.Literal(1),
    identity: CastIdentityV1,
    av_binding_key: Type.String({ minLength: 1 }),
    animation_id: Type.String({ minLength: 1 }),
  },
  { additionalProperties: false },
);
export type CastStopAnimV1 = Static<typeof CastStopAnimV1>;

/** ServerData 的四个独立 cast arms；完整联合由 server-data.ts 负责。 */
export const ServerDataCastSessionBeginArmV1 = Type.Union([
  sessionBeginArm(false, false, "cast_session_begin"),
  sessionBeginArm(false, true, "cast_session_begin"),
  sessionBeginArm(true, false, "cast_session_begin"),
  sessionBeginArm(true, true, "cast_session_begin"),
]);
// ServerData 只需要声明四个独立 arm；source/phase 的联合约束由 CastSyncV1
// canonical validator 执行，避免把 13 个交叉分支复制进总 ServerData union。
export const ServerDataCastSyncArmV1 = Type.Object(
  {
    ...CastSyncCommon,
    type: Type.Literal("cast_sync"),
    source: CastSourceV1,
    npc: Type.Boolean(),
    skill_id: Type.Union([Type.Null(), Type.String({ minLength: 1 })]),
    av_binding_key: Type.String({ minLength: 1 }),
    phase: CastPhaseV1,
    slot: CastSlotV1,
    outcome: CastOutcomeV1,
  },
  { additionalProperties: false },
);
export const ServerDataCastPlayAnimV1 = Type.Object(
  {
    ...CastPlayAnimV1.properties,
    type: Type.Literal("cast_play_anim"),
  },
  { additionalProperties: false },
);
export const ServerDataCastStopAnimV1 = Type.Object(
  {
    ...CastStopAnimV1.properties,
    type: Type.Literal("cast_stop_anim"),
  },
  { additionalProperties: false },
);

/** 运行时跨字段检查的轻量入口，供 contract pin 使用。 */
export function isCastSyncV1(value: unknown): value is CastSyncV1 {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  if (!Value.Check(CastSyncV1, value)) return false;
  const candidate = value as Record<string, unknown>;
  const identity = candidate.identity as Record<string, unknown>;
  return (
    isCanonicalUint64String(identity.session_generation) &&
    isCanonicalUint64String(identity.cast_instance_id)
  );
}
