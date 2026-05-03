import { Type, type Static } from "@sinclair/typebox";

import { Realm } from "./cultivation.js";
import { ZoneDeathKind } from "./death-lifecycle.js";
import { DeathInsightSpiritEyeV1 } from "./spirit-eye.js";
import { validate, type ValidationResult } from "./validate.js";

export const DeathInsightCategoryV1 = Type.Union(
  [
    Type.Literal("combat"),
    Type.Literal("cultivation"),
    Type.Literal("natural"),
    Type.Literal("tribulation"),
  ],
  { description: "遗念请求分类：战斗、修炼、老死、劫数" },
);
export type DeathInsightCategoryV1 = Static<typeof DeathInsightCategoryV1>;

export const DeathInsightPositionV1 = Type.Object(
  {
    x: Type.Number(),
    y: Type.Number(),
    z: Type.Number(),
  },
  { additionalProperties: false },
);
export type DeathInsightPositionV1 = Static<typeof DeathInsightPositionV1>;

/** plan-death-lifecycle-v1 §6/§7 — 死亡瞬间发给天道 agent 的遗念生成请求。 */
export const DeathInsightRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    request_id: Type.String({ minLength: 1, maxLength: 160 }),
    character_id: Type.String({ minLength: 1, maxLength: 160 }),
    at_tick: Type.Integer({ minimum: 0 }),
    cause: Type.String({ minLength: 1, maxLength: 512 }),
    category: DeathInsightCategoryV1,
    realm: Type.Optional(Realm),
    player_realm: Type.Optional(Type.String({ minLength: 1, maxLength: 64 })),
    zone_kind: ZoneDeathKind,
    death_count: Type.Integer({ minimum: 1 }),
    rebirth_chance: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    lifespan_remaining_years: Type.Optional(Type.Number({ minimum: 0 })),
    recent_biography: Type.Array(Type.String({ maxLength: 500 }), { maxItems: 64 }),
    position: Type.Optional(DeathInsightPositionV1),
    known_spirit_eyes: Type.Optional(Type.Array(DeathInsightSpiritEyeV1, { maxItems: 32 })),
    context: Type.Record(Type.String(), Type.Any()),
  },
  { additionalProperties: false },
);
export type DeathInsightRequestV1 = Static<typeof DeathInsightRequestV1>;

export function validateDeathInsightRequestV1Contract(data: unknown): ValidationResult {
  return validate(DeathInsightRequestV1, data);
}
