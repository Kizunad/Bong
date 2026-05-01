/**
 * 炼丹相关共享原子（plan-alchemy-v1 §4 数据契约）。
 *
 * 服务端 → 客户端的 alchemy_* 推送（炉体/会话/预测/丹书/丹毒/结算）走 server-data.ts；
 * 客户端 → 服务端的 alchemy_* 操作（开炉/投料/起炉/干预/翻页/学方/服丹）走 client-request.ts。
 * 本文件提供二者共享的原子类型。
 */
import { Type, type Static } from "@sinclair/typebox";

import { ColorKind } from "./cultivation.js";
import { type ValidationResult, validate } from "./validate.js";

const JS_SAFE_INTEGER_MAX = Number.MAX_SAFE_INTEGER;

export const BlockPosV1 = Type.Tuple([Type.Integer(), Type.Integer(), Type.Integer()]);
export type BlockPosV1 = Static<typeof BlockPosV1>;

/** 五结果桶 — 与 server::alchemy::outcome::OutcomeBucket 对齐。 */
export const AlchemyOutcomeBucket = Type.Union(
  [
    Type.Literal("perfect"),
    Type.Literal("good"),
    Type.Literal("flawed"),
    Type.Literal("waste"),
    Type.Literal("explode"),
  ],
  { description: "plan §1.3 结果分桶" },
);
export type AlchemyOutcomeBucket = Static<typeof AlchemyOutcomeBucket>;

/** 玩家干预（plan §1.3）— discriminated union by `kind`。 */
export const AlchemyInterventionV1 = Type.Union(
  [
    Type.Object(
      {
        kind: Type.Literal("adjust_temp"),
        temp: Type.Number({ minimum: 0, maximum: 1 }),
      },
      { additionalProperties: false },
    ),
    Type.Object(
      {
        kind: Type.Literal("inject_qi"),
        qi: Type.Number({ minimum: 0 }),
      },
      { additionalProperties: false },
    ),
    Type.Object(
      {
        kind: Type.Literal("auto_profile"),
        profile_id: Type.String(),
      },
      { additionalProperties: false },
    ),
  ],
  { description: "plan §1.3 Intervention" },
);
export type AlchemyInterventionV1 = Static<typeof AlchemyInterventionV1>;

/** 单条已学方子（与 client RecipeScrollStore.RecipeEntry 对齐）。 */
export const AlchemyRecipeEntryV1 = Type.Object(
  {
    id: Type.String(),
    display_name: Type.String(),
    body_text: Type.String({ maxLength: 4096 }),
    author: Type.String(),
    era: Type.String(),
    max_known: Type.Integer({ minimum: 1 }),
  },
  { additionalProperties: false },
);
export type AlchemyRecipeEntryV1 = Static<typeof AlchemyRecipeEntryV1>;

/** 中途投料阶段提示（plan §1.3）。 */
export const AlchemyStageHintV1 = Type.Object(
  {
    at_tick: Type.Integer({ minimum: 0 }),
    window: Type.Integer({ minimum: 0 }),
    summary: Type.String(),
    completed: Type.Boolean(),
    missed: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type AlchemyStageHintV1 = Static<typeof AlchemyStageHintV1>;

/** 干预 log 条目（带 tick）。 */
export const AlchemyInterventionLogV1 = Type.Object(
  {
    tick: Type.Integer({ minimum: 0 }),
    intervention: AlchemyInterventionV1,
  },
  { additionalProperties: false },
);
export type AlchemyInterventionLogV1 = Static<typeof AlchemyInterventionLogV1>;

/** 丹毒色快照（plan §2 — 复用 ColorKind）。 */
export const AlchemyContaminationLevelV1 = Type.Object(
  {
    color: ColorKind,
    current: Type.Number({ minimum: 0 }),
    max: Type.Number({ minimum: 0 }),
    ok: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type AlchemyContaminationLevelV1 = Static<typeof AlchemyContaminationLevelV1>;

export const AlchemySessionStartV1 = Type.Object(
  {
    v: Type.Literal(1),
    session_id: Type.String({ minLength: 1, maxLength: 256 }),
    recipe_id: Type.String({ minLength: 1, maxLength: 128 }),
    furnace_pos: BlockPosV1,
    furnace_tier: Type.Integer({ minimum: 1, maximum: 9 }),
    caster_id: Type.String({ minLength: 1, maxLength: 128 }),
    ts: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type AlchemySessionStartV1 = Static<typeof AlchemySessionStartV1>;

export const AlchemySessionEndV1 = Type.Object(
  {
    v: Type.Literal(1),
    session_id: Type.String({ minLength: 1, maxLength: 256 }),
    recipe_id: Type.Union([Type.String({ minLength: 1, maxLength: 128 }), Type.Null()]),
    furnace_pos: BlockPosV1,
    furnace_tier: Type.Integer({ minimum: 1, maximum: 9 }),
    caster_id: Type.String({ minLength: 1, maxLength: 128 }),
    bucket: AlchemyOutcomeBucket,
    pill: Type.Optional(Type.String({ minLength: 1, maxLength: 128 })),
    quality: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    damage: Type.Optional(Type.Number({ minimum: 0 })),
    meridian_crack: Type.Optional(Type.Number({ minimum: 0 })),
    elapsed_ticks: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
    ts: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type AlchemySessionEndV1 = Static<typeof AlchemySessionEndV1>;

export const AlchemyInterventionResultV1 = Type.Object(
  {
    v: Type.Literal(1),
    session_id: Type.String({ minLength: 1, maxLength: 256 }),
    recipe_id: Type.String({ minLength: 1, maxLength: 128 }),
    furnace_pos: BlockPosV1,
    caster_id: Type.String({ minLength: 1, maxLength: 128 }),
    intervention: AlchemyInterventionV1,
    temp_current: Type.Number({ minimum: 0, maximum: 1 }),
    qi_injected: Type.Number({ minimum: 0 }),
    accepted: Type.Boolean(),
    message: Type.Optional(Type.String({ maxLength: 512 })),
    ts: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type AlchemyInterventionResultV1 = Static<typeof AlchemyInterventionResultV1>;

export function validateAlchemySessionStartV1Contract(data: unknown): ValidationResult {
  return validate(AlchemySessionStartV1, data);
}

export function validateAlchemySessionEndV1Contract(data: unknown): ValidationResult {
  return validate(AlchemySessionEndV1, data);
}

export function validateAlchemyInterventionResultV1Contract(data: unknown): ValidationResult {
  return validate(AlchemyInterventionResultV1, data);
}
