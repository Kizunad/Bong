import { describe, expect, it } from "vitest";

import {
  PSEUDO_VEIN_NARRATION_TEMPLATES,
  pseudoVeinNarrationKeyFromSnapshot,
  renderPseudoVeinDissipateNarration,
  renderPseudoVeinSnapshotNarration,
} from "../src/narration/templates.js";

describe("pseudo vein narration templates", () => {
  it("exposes lure warning dissipate template keys required by the plan", () => {
    expect(Object.keys(PSEUDO_VEIN_NARRATION_TEMPLATES)).toEqual([
      "pseudo_vein.lure",
      "pseudo_vein.warning",
      "pseudo_vein.dissipate",
    ]);
  });

  it("selects lure warning dissipate by qi thresholds", () => {
    expect(pseudoVeinNarrationKeyFromSnapshot({ spirit_qi_current: 0.6 })).toBe("pseudo_vein.lure");
    expect(pseudoVeinNarrationKeyFromSnapshot({ spirit_qi_current: 0.4 })).toBe("pseudo_vein.lure");
    expect(pseudoVeinNarrationKeyFromSnapshot({ spirit_qi_current: 0.3 })).toBe("pseudo_vein.warning");
    expect(pseudoVeinNarrationKeyFromSnapshot({ spirit_qi_current: 0 })).toBe("pseudo_vein.dissipate");
  });

  it("renders zone-scoped warning narration without revealing trap mechanics", () => {
    const narration = renderPseudoVeinSnapshotNarration({
      v: 1,
      id: "pseudo_vein_42",
      center_xz: [1280, -640],
      spirit_qi_current: 0.3,
      occupants: ["offline:Azure"],
      spawned_at_tick: 1,
      estimated_decay_at_tick: 2,
      season_at_spawn: "summer_to_winter",
    });

    expect(narration).toEqual({
      scope: "zone",
      target: "pseudo_vein_42",
      text: PSEUDO_VEIN_NARRATION_TEMPLATES["pseudo_vein.warning"],
      style: "system_warning",
    });
    expect(narration.text).toContain("此处灵气，似有异变");
    expect(narration.text).not.toMatch(/陷阱|概率|30%|天劫/u);
  });

  it("renders dissipate narration from dissipate events", () => {
    const narration = renderPseudoVeinDissipateNarration({
      v: 1,
      id: "pseudo_vein_42",
      center_xz: [1280, -640],
      storm_anchors: [[1380, -650]],
      storm_duration_ticks: 9000,
      qi_redistribution: { refill_to_hungry_ring: 0.7, collected_by_tiandao: 0.3 },
    });

    expect(narration.scope).toBe("zone");
    expect(narration.target).toBe("pseudo_vein_42");
    expect(narration.style).toBe("narration");
    expect(narration.text).toContain("负压");
  });
});
