import { describe, expect, it } from "vitest";
import type { SeasonStateV1 } from "@bong/schema";
import {
  containsExplicitSeasonName,
  renderSeasonalNarration,
  SeasonalNarrationTracker,
} from "../src/templates/seasonal.js";

describe("seasonal narration", () => {
  it("seasonal_narration_no_explicit_name", () => {
    for (const season of ["summer", "summer_to_winter", "winter", "winter_to_summer"] as const) {
      const narration = renderSeasonalNarration(state(season));
      expect(containsExplicitSeasonName(narration.text)).toBe(false);
      expect(narration.text).not.toMatch(/炎汐|凝汐|汐转|季节/u);
    }
  });

  it("delays transition narration by at least one hundred ticks", () => {
    const tracker = new SeasonalNarrationTracker();
    expect(tracker.ingest(state("summer"), 1000)).toHaveLength(0);
    expect(tracker.ingest(state("winter"), 1001)).toHaveLength(0);
    expect(tracker.ingest(state("winter"), 1099)).toHaveLength(0);
    expect(tracker.ingest(state("winter"), 1305)).toHaveLength(1);
  });
});

function state(season: SeasonStateV1["season"]): SeasonStateV1 {
  return {
    season,
    tick_into_phase: 0,
    phase_total_ticks: 1000,
    year_index: 0,
  };
}
