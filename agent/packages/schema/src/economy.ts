// plan-economy-v1 P2/P3 — 骨币经济聚合 telemetry。

import { Type, type Static } from "@sinclair/typebox";

import { SeasonV1 } from "./world-state.js";
import { validate, type ValidationResult } from "./validate.js";

export const BoneCoinTickV1 = Type.Object(
  {
    v: Type.Literal(1),
    tick: Type.Integer({ minimum: 0 }),
    season: SeasonV1,
    total_spirit_qi: Type.Number({ minimum: 0 }),
    total_face_value: Type.Number({ minimum: 0 }),
    active_coin_count: Type.Integer({ minimum: 0 }),
    rotten_coin_count: Type.Integer({ minimum: 0 }),
    legacy_scalar_count: Type.Integer({ minimum: 0 }),
    rhythm_multiplier: Type.Number({ minimum: 0 }),
    market_factor: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BoneCoinTickV1 = Static<typeof BoneCoinTickV1>;

export const PriceSampleV1 = Type.Object(
  {
    item_id: Type.String({ minLength: 1 }),
    base_price: Type.Integer({ minimum: 0 }),
    final_price: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type PriceSampleV1 = Static<typeof PriceSampleV1>;

export const PriceIndexV1 = Type.Object(
  {
    v: Type.Literal(1),
    tick: Type.Integer({ minimum: 0 }),
    season: SeasonV1,
    supply_spirit_qi: Type.Number({ minimum: 0 }),
    demand_spirit_qi: Type.Number({ exclusiveMinimum: 0 }),
    rhythm_multiplier: Type.Number({ minimum: 0 }),
    market_factor: Type.Number({ minimum: 0 }),
    price_multiplier: Type.Number({ minimum: 0 }),
    sample_prices: Type.Array(PriceSampleV1),
  },
  { additionalProperties: false },
);
export type PriceIndexV1 = Static<typeof PriceIndexV1>;

export function validateBoneCoinTickV1Contract(
  data: unknown,
): ValidationResult {
  return validate(BoneCoinTickV1, data);
}

export function validatePriceIndexV1Contract(data: unknown): ValidationResult {
  return validate(PriceIndexV1, data);
}
