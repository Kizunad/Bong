import { Type, type Static } from "@sinclair/typebox";

import { ColorKind, InsightCategory, Realm } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

/** plan §6.2 insight-request — 真元染色状态摘要。 */
export const QiColorStateV1 = Type.Object(
  {
    main: ColorKind,
    secondary: Type.Optional(ColorKind),
    is_chaotic: Type.Boolean(),
    is_hunyuan: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type QiColorStateV1 = Static<typeof QiColorStateV1>;

export const InsightRequestV1 = Type.Object(
  {
    trigger_id: Type.String({ minLength: 1, maxLength: 128 }),
    character_id: Type.String({ minLength: 1 }),
    realm: Realm,
    qi_color_state: QiColorStateV1,
    recent_biography: Type.Array(Type.String({ maxLength: 500 }), { maxItems: 64 }),
    composure: Type.Number({ minimum: 0, maximum: 1 }),
    available_categories: Type.Array(InsightCategory, { maxItems: 7 }),
    global_caps: Type.Record(Type.String(), Type.Number()),
  },
  { additionalProperties: false },
);
export type InsightRequestV1 = Static<typeof InsightRequestV1>;

export function validateInsightRequestV1Contract(data: unknown): ValidationResult {
  return validate(InsightRequestV1, data);
}
