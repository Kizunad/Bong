import type { Narration, PriceIndexV1 } from "@bong/schema";

const NARRATION_COOLDOWN_TICKS = 12_000;
const PRICE_SPIKE_RATIO = 1.25;
const PRICE_DROP_RATIO = 0.8;

export class EconomyAnalyzer {
  private lastNarrationTick: number | null = null;
  private previousIndex: PriceIndexV1 | null = null;

  ingestPriceIndex(index: PriceIndexV1): Narration[] {
    const previous = this.previousIndex;
    this.previousIndex = index;

    if (!this.canNarrate(index.tick)) {
      return [];
    }

    const trend = previous ? priceTrend(previous, index) : "初账";
    return [
      {
        scope: "broadcast",
        style: "narration",
        text: renderEconomyNarration(index, trend),
      },
    ];
  }

  private canNarrate(tick: number): boolean {
    if (
      this.lastNarrationTick !== null &&
      tick - this.lastNarrationTick < NARRATION_COOLDOWN_TICKS
    ) {
      return false;
    }

    this.lastNarrationTick = tick;
    return true;
  }
}

type PriceTrend = "涨" | "跌" | "平" | "初账";

function priceTrend(previous: PriceIndexV1, current: PriceIndexV1): PriceTrend {
  if (previous.price_multiplier <= 0) {
    return "平";
  }

  const ratio = current.price_multiplier / previous.price_multiplier;
  if (ratio >= PRICE_SPIKE_RATIO) {
    return "涨";
  }
  if (ratio <= PRICE_DROP_RATIO) {
    return "跌";
  }
  return "平";
}

function renderEconomyNarration(index: PriceIndexV1, trend: PriceTrend): string {
  const total = formatQi(index.supply_spirit_qi);
  const multiplier = index.price_multiplier.toFixed(2);
  const season = seasonLabel(index.season);

  if (trend === "涨") {
    return `天下灵气总价 ${total}，${season}催得骨币发热，市价系数升至 ${multiplier}。守财的人又多算了一遍。`;
  }
  if (trend === "跌") {
    return `天下灵气总价 ${total}，${season}压住市声，市价系数落到 ${multiplier}。骨币还在，真元不肯等人。`;
  }
  if (trend === "初账") {
    return `天下灵气总价 ${total}，${season}开账，骨币市价系数 ${multiplier}。天道只记真元，不记面额。`;
  }
  return `天下灵气总价 ${total}，${season}未改大势，骨币市价系数 ${multiplier}。坐标、丹方和路，比钱更难买。`;
}

function formatQi(value: number): string {
  if (!Number.isFinite(value)) {
    return "0.0";
  }
  return value.toFixed(1);
}

function seasonLabel(season: PriceIndexV1["season"]): string {
  switch (season) {
    case "summer":
      return "炎汐";
    case "winter":
      return "凝汐";
    case "summer_to_winter":
    case "winter_to_summer":
      return "汐转";
  }
}
