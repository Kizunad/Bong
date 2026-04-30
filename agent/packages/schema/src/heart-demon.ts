import { Type, type Static } from "@sinclair/typebox";

import { InsightCategory, Realm } from "./cultivation.js";
import { QiColorStateV1 } from "./insight-request.js";
import { HeartDemonOfferChoiceV1 } from "./server-data.js";
import { validate, type ValidationResult } from "./validate.js";

export const HeartDemonPregenRequestV1 = Type.Object(
  {
    trigger_id: Type.String({ minLength: 1, maxLength: 128 }),
    character_id: Type.String({ minLength: 1 }),
    actor_name: Type.String({ minLength: 1, maxLength: 128 }),
    realm: Realm,
    qi_color_state: QiColorStateV1,
    recent_biography: Type.Array(Type.String({ maxLength: 500 }), { maxItems: 64 }),
    composure: Type.Number({ minimum: 0, maximum: 1 }),
    started_tick: Type.Integer({ minimum: 0 }),
    waves_total: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type HeartDemonPregenRequestV1 = Static<typeof HeartDemonPregenRequestV1>;

export const HeartDemonOfferDraftV1 = Type.Object(
  {
    offer_id: Type.String({ minLength: 1, maxLength: 128 }),
    trigger_id: Type.String({ minLength: 1, maxLength: 128 }),
    trigger_label: Type.String({ minLength: 1, maxLength: 128 }),
    realm_label: Type.String({ minLength: 1, maxLength: 128 }),
    composure: Type.Number({ minimum: 0, maximum: 1 }),
    quota_remaining: Type.Integer({ minimum: 0 }),
    quota_total: Type.Integer({ minimum: 1 }),
    expires_at_ms: Type.Integer({ minimum: 0 }),
    choices: Type.Array(HeartDemonOfferChoiceV1, { minItems: 1, maxItems: 4 }),
  },
  { additionalProperties: false },
);
export type HeartDemonOfferDraftV1 = Static<typeof HeartDemonOfferDraftV1>;

export type HeartDemonCanonicalChoiceKind = "steadfast" | "obsession" | "no_solution";

export const HEART_DEMON_CANONICAL_CHOICES: ReadonlyArray<{
  kind: HeartDemonCanonicalChoiceKind;
  choice_id: string;
  category: InsightCategory;
  title: string;
  effect_summary: string;
  style_hint: string;
}> = Object.freeze([
  {
    kind: "steadfast",
    choice_id: "heart_demon_choice_0",
    category: "Composure",
    title: "守本心",
    effect_summary: "稳住心神，回复少量当前真元",
    style_hint: "稳妥",
  },
  {
    kind: "obsession",
    choice_id: "heart_demon_choice_1",
    category: "Breakthrough",
    title: "斩执念",
    effect_summary: "若斩错心魔，将损当前真元并强化下一道开天雷",
    style_hint: "冒险",
  },
  {
    kind: "no_solution",
    choice_id: "heart_demon_choice_2",
    category: "Perception",
    title: "无解",
    effect_summary: "承认无解，不得增益也不受真元惩罚",
    style_hint: "止损",
  },
]);

export function validateHeartDemonPregenRequestV1Contract(
  data: unknown,
): ValidationResult {
  return validate(HeartDemonPregenRequestV1, data);
}

export function validateHeartDemonOfferDraftV1Contract(data: unknown): ValidationResult {
  return validate(HeartDemonOfferDraftV1, data);
}
