import { describe, expect, it } from "vitest";

import type { ChatSignal, WorldStateV1 } from "@bong/schema";

import { createMockWorldState } from "../src/mock-state.js";
import type { AgentDecision } from "../src/parse.js";
import { WorldModel } from "../src/world-model.js";

function withZoneSpiritQi(
  baseState: WorldStateV1,
  tick: number,
  zoneName: string,
  spiritQi: number,
): WorldStateV1 {
  return {
    ...baseState,
    tick,
    zones: baseState.zones.map((zone) =>
      zone.name === zoneName
        ? {
            ...zone,
            spirit_qi: spiritQi,
          }
        : zone,
    ),
  };
}

function buildSignal(player: string, raw: string, sentiment: number): ChatSignal {
  return {
    player,
    raw,
    sentiment,
    intent: sentiment < 0 ? "complaint" : "social",
    influence_weight: 0.5,
  };
}

function buildDecision(reasoning: string): AgentDecision {
  return {
    commands: [
      {
        type: "spawn_event",
        target: "blood_valley",
        params: {
          event: "thunder_tribulation",
          intensity: 0.6,
        },
      },
    ],
    narrations: [],
    reasoning,
  };
}

describe("WorldModel", () => {
  it.each([
    {
      trend: "rising",
      values: [0.18, 0.22, 0.26, 0.38, 0.42, 0.46],
    },
    {
      trend: "stable",
      values: [0.4, 0.41, 0.39, 0.4, 0.41, 0.4],
    },
    {
      trend: "falling",
      values: [0.82, 0.78, 0.74, 0.48, 0.44, 0.4],
    },
  ])("computes $trend zone trend from recent history", ({ trend, values }) => {
    const model = new WorldModel();
    const state = createMockWorldState();

    values.forEach((value, index) => {
      model.updateState(withZoneSpiritQi(state, 84_000 + index, "blood_valley", value));
    });

    const snapshot = model.getZoneTrendSnapshot("blood_valley");

    expect(snapshot.trend).toBe(trend);
    expect(model.getZoneTrend("blood_valley")).toBe(trend);
    expect(snapshot.samples).toBe(values.length);
  });

  it("stores latest state, chat signals, decisions, and bounded history deterministically", () => {
    const model = new WorldModel({
      maxZoneHistory: 3,
      maxChatSignals: 2,
    });
    const state = createMockWorldState();

    const updates = [0.2, 0.3, 0.4, 0.5].map((value, index) =>
      withZoneSpiritQi(state, 90_000 + index, "blood_valley", value),
    );

    for (const update of updates) {
      model.updateState(update);
    }

    const signals = [
      buildSignal("Steve", "灵气太少了", -0.6),
      buildSignal("Alex", "大家早", 0.2),
      buildSignal("NewPlayer1", "求助", -0.2),
    ];
    model.rememberChatSignals(signals);

    const decision = buildDecision("before mutation");
    model.rememberDecision("era", decision);
    model.rememberCurrentEra({
      name: "灵潮纪",
      sinceTick: 90_000,
      globalEffect: "all zones spirit_qi +0.01",
    });

    const latestUpdate = updates.at(-1);
    const latestSignal = signals.at(-1);

    if (!latestUpdate || !latestSignal) {
      throw new Error("expected seeded updates and signals");
    }

    latestUpdate.tick = 999_999;
    latestSignal.raw = "已被篡改";
    decision.reasoning = "after mutation";

    expect(model.latestState?.tick).toBe(90_003);
    expect(model.getZoneHistory("blood_valley").map((zone) => zone.spirit_qi)).toEqual([0.3, 0.4, 0.5]);
    expect(model.chatSignals.map((signal) => signal.player)).toEqual(["Alex", "NewPlayer1"]);
    expect(model.chatSignals[1]?.raw).toBe("求助");
    expect(model.lastDecisions.get("era")?.reasoning).toBe("before mutation");
    expect(model.currentEra).toEqual({
      name: "灵潮纪",
      sinceTick: 90_000,
      globalEffect: "all zones spirit_qi +0.01",
    });
    expect(model.getBalanceSnapshot()).toEqual(
      expect.objectContaining({
        tick: 90_003,
        playerCount: 3,
        summary: expect.stringContaining("Gini"),
        analysis: expect.objectContaining({
          severity: "severe",
          recommendations: expect.arrayContaining([
            expect.objectContaining({ kind: "pressure_strongest" }),
          ]),
        }),
      }),
    );
  });

  it("keeps retained era and balance snapshot immutable from caller mutations", () => {
    const model = new WorldModel();
    const state = createMockWorldState();
    model.updateState(state);

    const era = {
      name: "末法纪",
      sinceTick: 84_000,
      globalEffect: "all zones spirit_qi -0.02",
    };

    model.rememberCurrentEra(era);
    era.name = "已被篡改";

    const snapshot = model.getBalanceSnapshot();
    if (!snapshot) {
      throw new Error("expected balance snapshot");
    }

    snapshot.summary = "mutated externally";
    snapshot.analysis.severity = "balanced";

    const freshSnapshot = model.getBalanceSnapshot();
    expect(model.currentEra?.name).toBe("末法纪");
    expect(freshSnapshot?.summary).not.toBe("mutated externally");
    expect(freshSnapshot?.analysis.severity).toBe("severe");
  });
});
