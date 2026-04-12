import { Type, type Static } from "@sinclair/typebox";

import { InsightCategory } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

/** plan §5.2 — 效果枚举占位（详细 variant 与数值由 Arbiter 在 Rust 侧校验）。 */
export const InsightEffectKind = Type.String({
  minLength: 1,
  maxLength: 64,
  description: "apply_choice 支持的 InsightEffect variant 名，例如 `MeridianIntegrityBoost`",
});
export type InsightEffectKind = Static<typeof InsightEffectKind>;

export const InsightChoiceV1 = Type.Object(
  {
    category: InsightCategory,
    effect_kind: InsightEffectKind,
    magnitude: Type.Number(),
    flavor_text: Type.String({ maxLength: 500 }),
    narrator_voice: Type.Optional(Type.String({ maxLength: 64 })),
  },
  { additionalProperties: false },
);
export type InsightChoiceV1 = Static<typeof InsightChoiceV1>;

export const InsightOfferV1 = Type.Object(
  {
    offer_id: Type.String({ minLength: 1, maxLength: 128 }),
    trigger_id: Type.String({ minLength: 1, maxLength: 128 }),
    choices: Type.Array(InsightChoiceV1, { minItems: 1, maxItems: 4 }),
  },
  { additionalProperties: false },
);
export type InsightOfferV1 = Static<typeof InsightOfferV1>;

export function validateInsightOfferV1Contract(data: unknown): ValidationResult {
  return validate(InsightOfferV1, data);
}
