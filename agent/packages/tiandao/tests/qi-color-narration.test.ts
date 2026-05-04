import { describe, expect, it } from "vitest";

import {
  QiColorNarrationTracker,
  renderQiColorNarration,
} from "../src/qi-color-narration.js";
import type { ColorKind, PlayerProfile, WorldStateV1 } from "@bong/schema";
import { createTestWorldState } from "./support/fakes.js";

function player(color: {
  main: string;
  secondary?: string;
  chaotic?: boolean;
  hunyuan?: boolean;
}): PlayerProfile {
  return {
    uuid: "offline:Azure",
    name: "Azure",
    realm: "Induce",
    composite_power: 0.2,
    breakdown: {
      combat: 0.2,
      wealth: 0.2,
      social: 0.2,
      karma: 0,
      territory: 0.1,
    },
    trend: "stable",
    active_hours: 1,
    zone: "starter_zone",
    pos: [0, 64, 0],
    recent_kills: 0,
    recent_deaths: 0,
    cultivation: {
      realm: "Induce",
      qi_current: 10,
      qi_max: 20,
      qi_max_frozen: 0,
      meridians_opened: 3,
      meridians_total: 20,
      qi_color_main: color.main as ColorKind,
      qi_color_secondary: color.secondary as ColorKind | undefined,
      qi_color_chaotic: color.chaotic ?? false,
      qi_color_hunyuan: color.hunyuan ?? false,
      composure: 0.8,
    },
  };
}

function stateWith(playerProfile: PlayerProfile, tick: number): WorldStateV1 {
  return {
    ...createTestWorldState(),
    tick,
    players: [playerProfile],
  };
}

describe("QiColorNarrationTracker", () => {
  it("seeds first snapshot without narration", () => {
    const tracker = new QiColorNarrationTracker();

    expect(tracker.ingest(stateWith(player({ main: "Mellow" }), 1))).toEqual([]);
  });

  it("publishes narration when main color changes", () => {
    const tracker = new QiColorNarrationTracker();
    tracker.ingest(stateWith(player({ main: "Mellow" }), 1));

    const narrations = tracker.ingest(stateWith(player({ main: "Solid" }), 2));

    expect(narrations).toHaveLength(1);
    expect(narrations[0]).toMatchObject({
      scope: "player",
      target: "qi_color:offline:Azure|tick:2",
      style: "narration",
    });
    expect(narrations[0].text).toContain("温润色转为凝实色");
  });

  it("prioritizes chaotic and hunyuan transitions", () => {
    const tracker = new QiColorNarrationTracker();
    tracker.ingest(stateWith(player({ main: "Sharp" }), 10));

    const chaotic = tracker.ingest(stateWith(player({ main: "Sharp", chaotic: true }), 11));
    const hunyuan = tracker.ingest(stateWith(player({ main: "Sharp", hunyuan: true }), 12));

    expect(chaotic[0].text).toContain("杂乱");
    expect(hunyuan[0].text).toContain("混元");
  });

  it("renders contract-valid narration directly", () => {
    const narration = renderQiColorNarration(
      { uuid: "offline:Azure", name: "Azure" },
      { main: "Violent", chaotic: false, hunyuan: false },
      { main: "Heavy", chaotic: false, hunyuan: false },
      42,
    );

    expect(narration?.text).toContain("沉重色转为暴烈色");
  });
});
