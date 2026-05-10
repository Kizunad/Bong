import { describe, expect, it } from "vitest";

import type { PoisonDoseEventV1, PoisonOverdoseEventV1 } from "@bong/schema";
import {
  poisonSideEffectText,
  renderPoisonDoseNarration,
  renderPoisonOverdoseNarration,
} from "../src/poison-trait-runtime.js";

describe("poison trait narration", () => {
  it("renders all five side effect tags without modern phrasing", () => {
    const tags = [
      "qi_focus_drift_2h",
      "rage_burst_30min",
      "hallucin_tint_6h",
      "digest_lock_6h",
      "toxicity_tier_unlock",
    ] as const;
    for (const tag of tags) {
      const text = poisonSideEffectText(tag);
      expect(text.length).toBeGreaterThan(8);
      expect(text).not.toMatch(/buff|debuff|DPS|level/i);
    }
  });

  it("renders dose narration with toxicity and digestion numbers", () => {
    const event: PoisonDoseEventV1 = {
      v: 1,
      player_entity_id: 7,
      dose_amount: 15,
      side_effect_tag: "toxicity_tier_unlock",
      poison_level_after: 73,
      digestion_after: 90,
      at_tick: 120,
    };
    const narration = renderPoisonDoseNarration(event);
    expect(narration.target).toBe("poison_dose:7|tick:120");
    expect(narration.text).toContain("73");
    expect(narration.text).toContain("90");
    expect(narration.text).toContain("门槛");
  });

  it("renders overdose narration with lifespan and micro tear hint", () => {
    const event: PoisonOverdoseEventV1 = {
      v: 1,
      player_entity_id: 7,
      severity: "severe",
      overflow: 30,
      lifespan_penalty_years: 8,
      micro_tear_probability: 0.3,
      at_tick: 240,
    };
    const narration = renderPoisonOverdoseNarration(event);
    expect(narration.target).toBe("poison_overdose:7|tick:240");
    expect(narration.text).toContain("重度反噬");
    expect(narration.text).toMatch(/寿元|微裂/);
  });
});
