import { describe, expect, it } from "vitest";
import type { PriceIndexV1 } from "@bong/schema";
import { EconomyAnalyzer } from "../src/economy-analyzer.js";

function index(overrides: Partial<PriceIndexV1> = {}): PriceIndexV1 {
  return {
    v: 1,
    tick: 720_000,
    season: "summer_to_winter",
    supply_spirit_qi: 27.5,
    demand_spirit_qi: 50,
    rhythm_multiplier: 1.1,
    market_factor: 0.9,
    price_multiplier: 0.99,
    sample_prices: [{ item_id: "common_good", base_price: 4, final_price: 4 }],
    ...overrides,
  };
}

describe("EconomyAnalyzer", () => {
  it("emits monthly total-spirit narration on first price index", () => {
    const analyzer = new EconomyAnalyzer();

    expect(analyzer.ingestPriceIndex(index())).toEqual([
      expect.objectContaining({
        scope: "broadcast",
        text: expect.stringContaining("天下灵气总价 27.5"),
      }),
    ]);
  });

  it("uses cooldown and detects price rise after cooldown", () => {
    const analyzer = new EconomyAnalyzer();
    analyzer.ingestPriceIndex(index({ tick: 720_000, price_multiplier: 1.0 }));

    expect(
      analyzer.ingestPriceIndex(index({ tick: 721_000, price_multiplier: 1.5 })),
    ).toHaveLength(0);

    expect(
      analyzer.ingestPriceIndex(index({ tick: 740_000, price_multiplier: 2.0 })),
    ).toEqual([
      expect.objectContaining({
        text: expect.stringContaining("市价系数升至 2.00"),
      }),
    ]);
  });
});
