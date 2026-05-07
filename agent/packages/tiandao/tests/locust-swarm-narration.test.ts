import { describe, expect, it } from "vitest";
import type { RatPhaseChangeEventV1, WorldStateV1 } from "@bong/schema";
import {
  LOCUST_SWARM_COOLDOWN_TICKS,
  LocustSwarmNarrationTracker,
  parseRatPhaseEventFromRedis,
} from "../src/locust-swarm-narration.js";
import { createTestWorldState } from "./support/fakes.js";

function ratPhaseEvent(overrides: Partial<RatPhaseChangeEventV1> = {}): RatPhaseChangeEventV1 {
  return {
    chunk: [8, 8],
    zone: "starter_zone",
    group_id: 7,
    from: "solitary",
    to: { transitioning: { progress: 0 } },
    rat_count: 12,
    local_qi: 0.42,
    qi_gradient: 0.31,
    tick: 10_000,
    ...overrides,
  };
}

function worldState(overrides: Partial<WorldStateV1> = {}): WorldStateV1 {
  const base = createTestWorldState();
  return {
    ...base,
    tick: 10_000,
    zones: [
      {
        name: "starter_zone",
        spirit_qi: 0.72,
        danger_level: 2,
        active_events: [],
        player_count: 1,
      },
    ],
    rat_density_heatmap: {
      zones: {
        starter_zone: {
          total: 12,
          solitary: 2,
          transitioning: 10,
          gregarious: 0,
        },
      },
    },
    ...overrides,
  };
}

describe("LocustSwarmNarrationTracker", () => {
  it("parses_rat_phase_event_from_redis", () => {
    const event = ratPhaseEvent();

    expect(parseRatPhaseEventFromRedis(JSON.stringify(event))).toEqual(event);
    expect(parseRatPhaseEventFromRedis("{not json")).toBeNull();
    expect(parseRatPhaseEventFromRedis(JSON.stringify({ ...event, to: "unknown" }))).toBeNull();
  });

  it("escalates_to_locust_swarm_when_qi_zone_and_player_density_high", () => {
    const decision = new LocustSwarmNarrationTracker().ingest(ratPhaseEvent(), worldState());

    expect(decision.commands).toEqual([
      expect.objectContaining({
        type: "spawn_event",
        target: "starter_zone",
        params: expect.objectContaining({
          event: "beast_tide",
          tide_kind: "locust_swarm",
          target_zone: "starter_zone",
        }),
      }),
    ]);
    expect(decision.narrations).toEqual([
      expect.objectContaining({
        scope: "zone",
        target: "starter_zone",
        text: expect.stringContaining("灵蝗潮"),
      }),
    ]);
  });

  it("skips_escalation_when_calamity_in_progress", () => {
    const decision = new LocustSwarmNarrationTracker().ingest(
      ratPhaseEvent(),
      worldState({
        zones: [
          {
            name: "starter_zone",
            spirit_qi: 0.72,
            danger_level: 2,
            active_events: ["realm_collapse"],
            player_count: 1,
          },
        ],
      }),
    );

    expect(decision.commands).toHaveLength(0);
    expect(decision.narrations).toHaveLength(0);
    expect(decision.reasoning).toContain("calamity");
  });

  it("respects_24h_cooldown_per_target_zone", () => {
    const tracker = new LocustSwarmNarrationTracker();
    const first = tracker.ingest(ratPhaseEvent({ tick: 1_000 }), worldState({ tick: 1_000 }));
    const cooledDown = tracker.ingest(ratPhaseEvent({ tick: 1_100 }), worldState({ tick: 1_100 }));
    const afterCooldown = tracker.ingest(
      ratPhaseEvent({ tick: 1_000 + LOCUST_SWARM_COOLDOWN_TICKS }),
      worldState({ tick: 1_000 + LOCUST_SWARM_COOLDOWN_TICKS }),
    );

    expect(first.commands).toHaveLength(1);
    expect(cooledDown.commands).toHaveLength(0);
    expect(cooledDown.reasoning).toContain("cooldown");
    expect(afterCooldown.commands).toHaveLength(1);
  });
});
