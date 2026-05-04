import { describe, expect, it } from "vitest";

import {
  ZONG_STELE_FRAGMENTS,
  renderZongCoreActivationNarration,
  renderZongSteleNarration,
} from "../src/narration/zong_lore.js";

describe("plan-terrain-jiuzong-ruin-v1 zong lore narration", () => {
  it("keeps three unique stele fragments for every lost zong origin", () => {
    for (const [originId, fragments] of Object.entries(ZONG_STELE_FRAGMENTS)) {
      expect(Number(originId)).toBeGreaterThanOrEqual(1);
      expect(Number(originId)).toBeLessThanOrEqual(7);
      expect(fragments).toHaveLength(3);
      expect(new Set(fragments).size).toBe(3);
    }
  });

  it("renders formation activation as zone perception without global broadcast", () => {
    const narration = renderZongCoreActivationNarration({
      v: 1,
      zone_id: "jiuzong_bloodstream_ruin",
      core_id: "jiuzong_bloodstream_ruin:core:0",
      origin_id: 1,
      center_xz: [5500, -1000],
      activated_until_tick: 36000,
      base_qi: 0.4,
      active_qi: 0.6,
      charge_required: ["spirit_herb", "bone_coin", "true_qi"],
      narration_radius_blocks: 1000,
      anomaly_kind: 5,
    });

    expect(narration.scope).toBe("zone");
    expect(narration.target).toBe("jiuzong_bloodstream_ruin");
    expect(narration.style).toBe("perception");
    expect(narration.text).toContain("血溪故地灵脉异动");
    expect(narration.text).not.toMatch(/全服|概率|30 分钟/u);
  });

  it("wraps stele fragment index deterministically", () => {
    const narration = renderZongSteleNarration(7, 4, "jiuzong_youan_ruin");

    expect(narration.scope).toBe("zone");
    expect(narration.target).toBe("jiuzong_youan_ruin");
    expect(narration.text).toBe(ZONG_STELE_FRAGMENTS[7][1]);
  });
});
