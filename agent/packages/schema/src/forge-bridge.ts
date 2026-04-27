import { Type, type Static } from "@sinclair/typebox";

import { ColorKind } from "./cultivation.js";
import { ForgeOutcomeBucket } from "./forge.js";

const JS_SAFE_INTEGER_MAX = Number.MAX_SAFE_INTEGER;

export const ForgeMaterialStackV1 = Type.Object(
  {
    material: Type.String({ minLength: 1, maxLength: 128 }),
    count: Type.Integer({ minimum: 1, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type ForgeMaterialStackV1 = Static<typeof ForgeMaterialStackV1>;

export const ForgeStartPayloadV1 = Type.Object(
  {
    v: Type.Literal(1),
    session_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
    blueprint_id: Type.String({ minLength: 1, maxLength: 128 }),
    station_id: Type.String({ minLength: 1, maxLength: 128 }),
    caster_id: Type.String({ minLength: 1, maxLength: 128 }),
    materials: Type.Array(ForgeMaterialStackV1),
    ts: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type ForgeStartPayloadV1 = Static<typeof ForgeStartPayloadV1>;

export const ForgeOutcomePayloadV1 = Type.Object(
  {
    v: Type.Literal(1),
    session_id: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
    blueprint_id: Type.String({ minLength: 1, maxLength: 128 }),
    bucket: ForgeOutcomeBucket,
    weapon_item: Type.Optional(Type.String({ minLength: 1, maxLength: 128 })),
    quality: Type.Number({ minimum: 0, maximum: 1 }),
    color: Type.Optional(ColorKind),
    side_effects: Type.Array(Type.String({ minLength: 1, maxLength: 128 })),
    achieved_tier: Type.Integer({ minimum: 0, maximum: 4 }),
    caster_id: Type.String({ minLength: 1, maxLength: 128 }),
    ts: Type.Integer({ minimum: 0, maximum: JS_SAFE_INTEGER_MAX }),
  },
  { additionalProperties: false },
);
export type ForgeOutcomePayloadV1 = Static<typeof ForgeOutcomePayloadV1>;
