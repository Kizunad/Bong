import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

/**
 * plan-craft-v1 §3 — IPC schema 5 sample（agent / TypeScript 端 v1）。
 *
 * 与 server `server/src/schema/craft.rs` 1:1 镜像；TypeBox 是 source of truth，
 * 编译期自动派生 JSON Schema 供 server serde 校验。
 *
 * 5 sample：
 *   1. CraftStartReqV1       — client → server，玩家点 [开始手搓]（agent 端通常不需要）
 *   2. CraftSessionStateV1   — server → client，进度同步（agent 端通常不需要）
 *   3. CraftOutcomeV1        — server → agent，出炉广播（narration 输入）
 *   4. RecipeUnlockedV1      — server → agent，三渠道解锁广播（narration 输入）
 *   5. RecipeListV1          — server → client，配方表全表
 */

const RecipeId = Type.String({ minLength: 1 });
const PlayerId = Type.String({ minLength: 1 });

export const CraftCategoryV1 = Type.Union([
  Type.Literal("anqi_carrier"),
  Type.Literal("dugu_potion"),
  Type.Literal("tuike_skin"),
  Type.Literal("zhenfa_trap"),
  Type.Literal("tool"),
  Type.Literal("container"),
  Type.Literal("poison_powder"),
  Type.Literal("misc"),
]);
export type CraftCategoryV1 = Static<typeof CraftCategoryV1>;

export const CraftFailureReasonV1 = Type.Union([
  Type.Literal("player_cancelled"),
  Type.Literal("player_died"),
  Type.Literal("internal_error"),
]);
export type CraftFailureReasonV1 = Static<typeof CraftFailureReasonV1>;

export const InsightTriggerV1 = Type.Union([
  Type.Literal("breakthrough"),
  Type.Literal("near_death"),
  Type.Literal("defeat_stronger"),
]);
export type InsightTriggerV1 = Static<typeof InsightTriggerV1>;

export const UnlockEventSourceV1 = Type.Union([
  Type.Object(
    {
      kind: Type.Literal("scroll"),
      item_template: Type.String({ minLength: 1 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("mentor"),
      npc_archetype: Type.String({ minLength: 1 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("insight"),
      trigger: InsightTriggerV1,
    },
    { additionalProperties: false },
  ),
]);
export type UnlockEventSourceV1 = Static<typeof UnlockEventSourceV1>;

const ColorKind = Type.String({ minLength: 1 });
const Realm = Type.String({ minLength: 1 });

export const CraftRequirementsV1 = Type.Object(
  {
    realm_min: Type.Optional(Realm),
    qi_color_min: Type.Optional(Type.Tuple([ColorKind, Type.Number({ minimum: 0, maximum: 1 })])),
    skill_lv_min: Type.Optional(Type.Integer({ minimum: 0, maximum: 255 })),
  },
  { additionalProperties: false },
);
export type CraftRequirementsV1 = Static<typeof CraftRequirementsV1>;

export const CraftRecipeEntryV1 = Type.Object(
  {
    id: RecipeId,
    category: CraftCategoryV1,
    display_name: Type.String({ minLength: 1 }),
    materials: Type.Array(Type.Tuple([Type.String({ minLength: 1 }), Type.Integer({ minimum: 0 })])),
    qi_cost: Type.Number({ minimum: 0 }),
    time_ticks: Type.Integer({ minimum: 0 }),
    output: Type.Tuple([Type.String({ minLength: 1 }), Type.Integer({ minimum: 0 })]),
    requirements: CraftRequirementsV1,
    unlocked: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type CraftRecipeEntryV1 = Static<typeof CraftRecipeEntryV1>;

// ─── 5 sample ─────────────────────────────────────────────────────────

/** client → server：玩家点 [开始手搓]。 */
export const CraftStartReqV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_id: PlayerId,
    recipe_id: RecipeId,
    ts: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type CraftStartReqV1 = Static<typeof CraftStartReqV1>;

/** server → client：当前任务进度（每秒推 + 状态切换时推）。 */
export const CraftSessionStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_id: PlayerId,
    active: Type.Boolean(),
    recipe_id: Type.Optional(RecipeId),
    elapsed_ticks: Type.Integer({ minimum: 0 }),
    total_ticks: Type.Integer({ minimum: 0 }),
    ts: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type CraftSessionStateV1 = Static<typeof CraftSessionStateV1>;

/** server → agent / client：出炉结果（成功 / 失败二选一）。 */
export const CraftOutcomeV1 = Type.Union([
  Type.Object(
    {
      kind: Type.Literal("completed"),
      v: Type.Literal(1),
      player_id: PlayerId,
      recipe_id: RecipeId,
      output_template: Type.String({ minLength: 1 }),
      output_count: Type.Integer({ minimum: 0 }),
      completed_at_tick: Type.Integer({ minimum: 0 }),
      ts: Type.Integer({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      kind: Type.Literal("failed"),
      v: Type.Literal(1),
      player_id: PlayerId,
      recipe_id: RecipeId,
      reason: CraftFailureReasonV1,
      material_returned: Type.Integer({ minimum: 0 }),
      qi_refunded: Type.Number({ minimum: 0 }),
      ts: Type.Integer({ minimum: 0 }),
    },
    { additionalProperties: false },
  ),
]);
export type CraftOutcomeV1 = Static<typeof CraftOutcomeV1>;

/** server → agent / client：三渠道解锁广播（首学 / 师承 / 顿悟）。 */
export const RecipeUnlockedV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_id: PlayerId,
    recipe_id: RecipeId,
    source: UnlockEventSourceV1,
    unlocked_at_tick: Type.Integer({ minimum: 0 }),
    ts: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type RecipeUnlockedV1 = Static<typeof RecipeUnlockedV1>;

/** server → client：玩家上线时一次性推全配方表。 */
export const RecipeListV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_id: PlayerId,
    recipes: Type.Array(CraftRecipeEntryV1),
    ts: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type RecipeListV1 = Static<typeof RecipeListV1>;

// ─── validators ───────────────────────────────────────────────────────

export function validateCraftStartReqV1Contract(data: unknown): ValidationResult {
  return validate(CraftStartReqV1, data);
}

export function validateCraftSessionStateV1Contract(data: unknown): ValidationResult {
  return validate(CraftSessionStateV1, data);
}

export function validateCraftOutcomeV1Contract(data: unknown): ValidationResult {
  return validate(CraftOutcomeV1, data);
}

export function validateRecipeUnlockedV1Contract(data: unknown): ValidationResult {
  return validate(RecipeUnlockedV1, data);
}

export function validateRecipeListV1Contract(data: unknown): ValidationResult {
  return validate(RecipeListV1, data);
}
