import type { Narration, SeasonStateV1 } from "@bong/schema";

export type SeasonalNarrationSeason = SeasonStateV1["season"];

export const SEASONAL_NARRATION_TEMPLATES: Record<SeasonalNarrationSeason, string> = {
  summer: "灵气像热油贴地翻涌，草木和人心都比昨日急。",
  winter: "万物收声，冰下偶有细光一闪，很快又沉回土里。",
  summer_to_winter: "风向一夜乱了三次，云影挤在山口，像在等谁先失手。",
  winter_to_summer: "冻土底下有细响传开，旧雪未消，草根先醒了一寸。",
};

const EXPLICIT_SEASON_WORDS = ["炎汐", "凝汐", "汐转", "夏", "冬", "季节"];

export function renderSeasonalNarration(
  state: Pick<SeasonStateV1, "season">,
  target = "world:season",
): Narration {
  return {
    scope: "broadcast",
    target,
    style: "narration",
    text: SEASONAL_NARRATION_TEMPLATES[state.season],
  };
}

export function containsExplicitSeasonName(text: string): boolean {
  return EXPLICIT_SEASON_WORDS.some((word) => text.includes(word));
}

export class SeasonalNarrationTracker {
  private lastSeason: SeasonalNarrationSeason | null = null;
  private pending: { state: SeasonStateV1; readyAtTick: number } | null = null;

  ingest(state: SeasonStateV1, sourceTick: number): Narration[] {
    if (!state || !Number.isFinite(sourceTick)) {
      return [];
    }
    const tick = Math.max(0, Math.floor(sourceTick));
    if (this.lastSeason === null) {
      this.lastSeason = state.season;
      return [];
    }
    if (state.season !== this.lastSeason) {
      this.lastSeason = state.season;
      this.pending = {
        state,
        readyAtTick: tick + deterministicDelayTicks(state.season, tick),
      };
      return [];
    }
    if (this.pending && this.pending.state.season === state.season && tick >= this.pending.readyAtTick) {
      const narration = renderSeasonalNarration(this.pending.state);
      this.pending = null;
      return containsExplicitSeasonName(narration.text) ? [] : [narration];
    }
    return [];
  }
}

function deterministicDelayTicks(season: SeasonalNarrationSeason, sourceTick: number): number {
  let seed = sourceTick;
  for (const char of season) {
    seed = (seed * 33 + char.charCodeAt(0)) % 201;
  }
  return 100 + seed;
}
