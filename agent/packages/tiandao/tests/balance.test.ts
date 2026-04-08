import { describe, expect, it } from "vitest";

import { balanceAdvice, giniCoefficient } from "../src/balance.js";
import { createMockWorldState } from "../src/mock-state.js";

describe("giniCoefficient", () => {
  it("returns deterministic values for canonical power distributions", () => {
    expect(giniCoefficient([])).toBe(0);
    expect(giniCoefficient([1, 1, 1])).toBeCloseTo(0, 6);
    expect(giniCoefficient([0, 0, 1])).toBeCloseTo(2 / 3, 6);
  });
});

describe("balanceAdvice", () => {
  it("produces structured advice from player profiles", () => {
    const analysis = balanceAdvice(createMockWorldState().players);

    expect(analysis.gini).toBeGreaterThan(0.4);
    expect(analysis.severity).toBe("severe");
    expect(analysis.strongPlayers.map((player) => player.name)).toEqual(["Steve"]);
    expect(analysis.weakPlayers.map((player) => player.name)).toEqual(["NewPlayer1"]);
    expect(analysis.dominantZones).toEqual(["blood_valley"]);
    expect(analysis.recommendations).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: "pressure_strongest",
          targets: ["Steve"],
        }),
        expect.objectContaining({
          kind: "support_weaker_players",
          targets: ["newbie_valley"],
        }),
        expect.objectContaining({
          kind: "watch_dominant_zones",
          targets: ["blood_valley"],
        }),
      ]),
    );
  });

  it("gracefully degrades when there are no players", () => {
    const analysis = balanceAdvice([]);

    expect(analysis).toEqual({
      gini: 0,
      severity: "balanced",
      strongPlayers: [],
      weakPlayers: [],
      dominantZones: [],
      recommendations: [
        {
          kind: "maintain_balance",
          targets: [],
          summary: "维持当前平衡",
        },
      ],
    });
  });
});
