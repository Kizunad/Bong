/**
 * 炼丹相关共享原子（plan-alchemy-v1 §4 数据契约）。
 *
 * 服务端 → 客户端的 alchemy_* 推送（炉体/会话/预测/丹书/丹毒/结算）走 server-data.ts；
 * 客户端 → 服务端的 alchemy_* 操作（开炉/投料/起炉/干预/翻页/学方/服丹）走 client-request.ts。
 * 本文件提供二者共享的原子类型。
 */
import { Type, type Static } from "@sinclair/typebox";

import { ColorKind } from "./cultivation.js";

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
