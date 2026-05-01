/**
 * plan-skill-v1 §8 子技能 IPC schema。与 `server/src/schema/skill.rs` 1:1 对齐。
 *
 * 四条 channel 的 payload 定义（与 `channels.ts` 中 `SKILL_*` 对应）：
 * - `bong:skill/xp_gain` → SkillXpGainPayloadV1
 * - `bong:skill/lv_up` → SkillLvUpPayloadV1
 * - `bong:skill/cap_changed` → SkillCapChangedPayloadV1
 * - `bong:skill/scroll_used` → SkillScrollUsedPayloadV1
 *
 * 每份 payload 首字段均为 `v: 1`（版本锚），允许未来演进时 Rust 侧通过 serde::Deserialize 识别版本。
 */
import { Type, type Static } from "@sinclair/typebox";
import { validate, type ValidationResult } from "./validate.js";

/** plan §1 首批 skill + plan-cross-system-patch-v1 P1 跨系统熟练度。 */
export const SkillIdV1 = Type.Union([
  Type.Literal("herbalism"),
  Type.Literal("alchemy"),
  Type.Literal("forging"),
  Type.Literal("combat"),
  Type.Literal("mineral"),
  Type.Literal("cultivation"),
]);
export type SkillIdV1 = Static<typeof SkillIdV1>;

/**
 * plan §8 XpGainSource — discriminated union by `type`。
 * plan §3.1/§3.2/§3.3 四路来源 agent 据此区分"做中学 vs 顿悟 vs 突破 vs 师承"。
 */
export const XpGainSourceV1 = Type.Union(
  [
    Type.Object(
      {
        type: Type.Literal("action"),
        plan_id: Type.String({ minLength: 1 }),
        action: Type.String({ minLength: 1 }),
      },
      { additionalProperties: false },
    ),
    Type.Object(
      {
        type: Type.Literal("scroll"),
        scroll_id: Type.String({ minLength: 1 }),
        xp_grant: Type.Integer({ minimum: 0 }),
      },
      { additionalProperties: false },
    ),
    Type.Object(
      {
        type: Type.Literal("realm_breakthrough"),
      },
      { additionalProperties: false },
    ),
    Type.Object(
      {
        type: Type.Literal("mentor"),
        mentor_char: Type.Integer({ minimum: 0 }),
      },
      { additionalProperties: false },
    ),
  ],
  { description: "plan §8 XP 来源 tagged union" },
);
export type XpGainSourceV1 = Static<typeof XpGainSourceV1>;

/** plan §8 `SkillXpGain` 事件 → `bong:skill/xp_gain` 通道 payload。 */
export const SkillXpGainPayloadV1 = Type.Object(
  {
    v: Type.Literal(1),
    char_id: Type.Integer({ minimum: 0 }),
    skill: SkillIdV1,
    amount: Type.Integer({ minimum: 0 }),
    source: XpGainSourceV1,
  },
  { additionalProperties: false },
);
export type SkillXpGainPayloadV1 = Static<typeof SkillXpGainPayloadV1>;

/**
 * plan §8 `SkillLvUp` → `bong:skill/lv_up` 通道 payload。
 * narration 字段**不在此** —— plan §2.3 / §9 P5：agent 消费此事件后独立生成冷漠古意 narration。
 */
export const SkillLvUpPayloadV1 = Type.Object(
  {
    v: Type.Literal(1),
    char_id: Type.Integer({ minimum: 0 }),
    skill: SkillIdV1,
    new_lv: Type.Integer({ minimum: 0, maximum: 10 }),
  },
  { additionalProperties: false },
);
export type SkillLvUpPayloadV1 = Static<typeof SkillLvUpPayloadV1>;

export function validateSkillLvUpPayloadV1Contract(data: unknown): ValidationResult {
  return validate(SkillLvUpPayloadV1, data);
}

/** plan §4 境界软挂钩 cap 变动 → `bong:skill/cap_changed`。 */
export const SkillCapChangedPayloadV1 = Type.Object(
  {
    v: Type.Literal(1),
    char_id: Type.Integer({ minimum: 0 }),
    skill: SkillIdV1,
    new_cap: Type.Integer({ minimum: 0, maximum: 10 }),
  },
  { additionalProperties: false },
);
export type SkillCapChangedPayloadV1 = Static<typeof SkillCapChangedPayloadV1>;

export function validateSkillCapChangedPayloadV1Contract(data: unknown): ValidationResult {
  return validate(SkillCapChangedPayloadV1, data);
}

/**
 * plan §3.2 残卷使用结算 → `bong:skill/scroll_used`。
 * `was_duplicate=true` 时 `xp_granted=0`（scroll 不消耗，tooltip 提示"此卷已悟"）。
 */
export const SkillScrollUsedPayloadV1 = Type.Object(
  {
    v: Type.Literal(1),
    char_id: Type.Integer({ minimum: 0 }),
    scroll_id: Type.String({ minLength: 1 }),
    skill: SkillIdV1,
    xp_granted: Type.Integer({ minimum: 0 }),
    was_duplicate: Type.Boolean(),
    hydration: Type.Optional(Type.Boolean()),
  },
  { additionalProperties: false },
);
export type SkillScrollUsedPayloadV1 = Static<typeof SkillScrollUsedPayloadV1>;

export function validateSkillScrollUsedPayloadV1Contract(data: unknown): ValidationResult {
  return validate(SkillScrollUsedPayloadV1, data);
}

export const SkillEntrySnapshotV1 = Type.Object(
  {
    lv: Type.Integer({ minimum: 0, maximum: 10 }),
    xp: Type.Integer({ minimum: 0 }),
    xp_to_next: Type.Integer({ minimum: 1 }),
    total_xp: Type.Integer({ minimum: 0 }),
    cap: Type.Integer({ minimum: 0, maximum: 10 }),
    recent_gain_xp: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SkillEntrySnapshotV1 = Static<typeof SkillEntrySnapshotV1>;

export const SkillSnapshotPayloadV1 = Type.Object(
  {
    v: Type.Literal(1),
    char_id: Type.Integer({ minimum: 0 }),
    skills: Type.Record(Type.String({ minLength: 1 }), SkillEntrySnapshotV1),
    consumed_scrolls: Type.Array(Type.String({ minLength: 1 })),
  },
  { additionalProperties: false },
);
export type SkillSnapshotPayloadV1 = Static<typeof SkillSnapshotPayloadV1>;
