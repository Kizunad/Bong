import { describe, expect, it } from "vitest";

import {
  POI_NOVICE_NARRATION_TEMPLATES,
  renderPoiSpawnedNarration,
  renderTrespassNarration,
} from "../src/narration/templates.js";

describe("plan-poi-novice-v1 narration templates", () => {
  it("exposes spawned scroll and trespass template keys", () => {
    expect(Object.keys(POI_NOVICE_NARRATION_TEMPLATES)).toEqual([
      "poi_novice.spawned",
      "poi_novice.scroll_found",
      "poi_novice.trespass",
    ]);
  });

  it("renders POI spawned narration as silent zone perception", () => {
    const narration = renderPoiSpawnedNarration({
      v: 1,
      kind: "poi_spawned",
      poi_id: "spawn:forge_station",
      poi_type: "forge_station",
      zone: "spawn",
      pos: [304, 71, 208],
      selection_strategy: "strict_radius_1500",
      qi_affinity: 0.15,
      danger_bias: 0,
    });

    expect(narration).toEqual({
      scope: "zone",
      target: "spawn",
      text: POI_NOVICE_NARRATION_TEMPLATES["poi_novice.spawned"],
      style: "perception",
    });
    expect(narration.text).not.toMatch(/任务|面板|UI|标记/u);
  });

  it("renders trespass narration as one-week refusal warning", () => {
    const narration = renderTrespassNarration({
      v: 1,
      kind: "trespass",
      village_id: "spawn:rogue_village",
      player_id: "offline:Azure",
      killed_npc_count: 3,
      refusal_until_wall_clock_secs: 1770000000,
    });

    expect(narration.scope).toBe("zone");
    expect(narration.target).toBe("spawn:rogue_village");
    expect(narration.style).toBe("system_warning");
    expect(narration.text).toContain("一周");
  });
});
