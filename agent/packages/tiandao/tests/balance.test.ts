import { describe, expect, it } from "vitest";
import type { PlayerProfile } from "@bong/schema";
import { giniCoefficient, summarizeBalance } from "../src/balance.js";

interface PlayerOverrides extends Partial<Omit<PlayerProfile, "breakdown" | "pos" | "name">> {
  breakdown?: Partial<PlayerProfile["breakdown"]>;
  pos?: PlayerProfile["pos"];
}

function createPlayer(name: string, overrides: PlayerOverrides = {}): PlayerProfile {
  return {
    uuid: overrides.uuid ?? `offline:${name}`,
    name,
    realm: overrides.realm ?? "qi_refining_1",
    composite_power: overrides.composite_power ?? 0.2,
    breakdown: {
      combat: 0.2,
      wealth: 0.2,
      social: 0.2,
      karma: 0,
      territory: 0.2,
      ...overrides.breakdown,
    },
    trend: overrides.trend ?? "stable",
    active_hours: overrides.active_hours ?? 1,
    zone: overrides.zone ?? "starter_zone",
    pos: overrides.pos ?? [0, 64, 0],
    recent_kills: overrides.recent_kills ?? 0,
    recent_deaths: overrides.recent_deaths ?? 0,
  };
}

describe("balance", () => {
  it("calculates the standard gini coefficient for uneven power distributions", () => {
    expect(giniCoefficient([0, 0, 0.1, 1])).toBeCloseTo(0.704545, 6);
    expect(giniCoefficient([0.3, 0.3, 0.3])).toBeCloseTo(0, 6);
  });

  it("produces critical balance advice from strong and weak clusters", () => {
    const summary = summarizeBalance([
      createPlayer("Steve", { composite_power: 1, zone: "blood_valley" }),
      createPlayer("DiscipleA", { composite_power: 0.1, zone: "newbie_valley" }),
      createPlayer("DiscipleB", { composite_power: 0, zone: "newbie_valley" }),
      createPlayer("DiscipleC", { composite_power: 0, zone: "newbie_valley" }),
    ]);

    expect(summary.severity).toBe("critical");
    expect(summary.severityLabel).toBe("严重失衡");
    expect(summary.gini).toBeGreaterThan(0.55);
    expect(summary.strongPlayers.map((player) => player.name)).toEqual(["Steve"]);
    expect(summary.weakPlayers.map((player) => player.name)).toEqual([
      "DiscipleB",
      "DiscipleC",
      "DiscipleA",
    ]);
    expect(summary.weakestZone).toBe("newbie_valley");
    expect(summary.advice).toContain("对 Steve 施压");
    expect(summary.advice).toContain("newbie_valley 增加机缘密度");
  });
});
