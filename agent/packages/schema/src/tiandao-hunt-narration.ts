/**
 * plan-tiandao-hunt-v1 P3 — 天道狩猎专属叙事请求 IPC schema。
 */
import { Type, type Static } from "@sinclair/typebox";

import { Realm } from "./cultivation.js";
import { validate, type ValidationResult } from "./validate.js";

export const TiandaoHuntResponseLevelV1 = Type.Union(
  [
    Type.Literal("watch"),
    Type.Literal("pressure"),
    Type.Literal("tribulation"),
    Type.Literal("annihilate"),
  ],
  { description: "天道狩猎叙事响应档；none 不触发叙事请求" },
);
export type TiandaoHuntResponseLevelV1 = Static<typeof TiandaoHuntResponseLevelV1>;

export const TiandaoHuntNarrationRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    character_id: Type.String({ minLength: 1, maxLength: 128 }),
    realm: Realm,
    attention_level: Type.Number({ minimum: 0, maximum: 100 }),
    response_level: TiandaoHuntResponseLevelV1,
    zone: Type.String({ minLength: 1, maxLength: 128 }),
    recent_actions: Type.Array(Type.String({ minLength: 1, maxLength: 128 }), {
      minItems: 0,
      maxItems: 8,
    }),
    narration_count: Type.Integer({ minimum: 0, maximum: 65535 }),
  },
  { additionalProperties: false },
);
export type TiandaoHuntNarrationRequestV1 = Static<
  typeof TiandaoHuntNarrationRequestV1
>;

export function validateTiandaoHuntNarrationRequestV1Contract(
  data: unknown,
): ValidationResult {
  return validate(TiandaoHuntNarrationRequestV1, data);
}
