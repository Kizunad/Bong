import { describe, expect, it } from "vitest";
import type { Command, WorldStateV1, ZoneSnapshot } from "@bong/schema";
import { validateAgentCommandV1Contract } from "@bong/schema";
import { produceDeterministicNpcDecisions } from "../src/npc-producer.js";
import { WorldModel } from "../src/world-model.js";
import { createTestWorldState } from "./support/fakes.js";

function zone(name: string, spiritQi: number, overrides: Partial<ZoneSnapshot> = {}): ZoneSnapshot {
  return {
    name,
    spirit_qi: spiritQi,
    danger_level: overrides.danger_level ?? 1,
    active_events: overrides.active_events ?? [],
    player_count: overrides.player_count ?? 0,
  };
}

function produce(state: WorldStateV1, worldModel?: WorldModel) {
  return produceDeterministicNpcDecisions({
    state,
    worldModel,
    sourcedDecisions: [],
    metadata: { sourceTick: state.tick, correlationId: `tiandao-tick-${state.tick}` },
  });
}

function firstCommand(state: WorldStateV1, worldModel?: WorldModel): Command | undefined {
  return produce(state, worldModel)[0]?.decision.commands[0];
}

describe("deterministic NPC producer", () => {
  it("spawns rogue batch when spirit qi rises in a high-qi zone", () => {
    const previous = createTestWorldState();
    previous.zones = [zone("green_cloud_peak", 0.68, { player_count: 1 })];
    const current = createTestWorldState();
    current.tick = 1_200;
    current.zones = [zone("green_cloud_peak", 0.86, { player_count: 1 })];

    const model = new WorldModel();
    model.updateState(previous);
    model.updateState(current);

    const command = firstCommand(current, model);
    expect(command).toMatchObject({
      type: "spawn_npc",
      target: "green_cloud_peak",
      params: expect.objectContaining({
        archetype: "rogue",
        count: 2,
        reason: "spirit_qi_fluctuation",
      }),
    });
  });

  it("does not spawn rogues every tick or when zone already has enough rogues", () => {
    const state = createTestWorldState();
    state.tick = 1_201;
    state.zones = [zone("green_cloud_peak", 0.9)];
    expect(produce(state)).toEqual([]);

    state.tick = 1_200;
    state.npcs = Array.from({ length: 12 }, (_, index) => ({
      id: `npc_${index}v1`,
      kind: "minecraft:zombie",
      pos: [0, 64, 0],
      state: "idle",
      blackboard: {},
      digest: {
        archetype: "rogue",
        age_band: "adult",
        age_ratio: 0.2,
      },
    }));
    expect(produce(state)).toEqual([]);
  });

  it("produces faction event when era agent declares a new era", () => {
    const state = createTestWorldState();
    state.tick = 2_400;
    state.factions = [
      { id: "attack", loyalty_bias: 0.8, mission_queue: { pending_count: 2 } },
      { id: "defend", loyalty_bias: 0.3, mission_queue: { pending_count: 0 } },
    ];

    const decisions = produceDeterministicNpcDecisions({
      state,
      sourcedDecisions: [
        {
          source: "era",
          decision: {
            commands: [],
            narrations: [{ scope: "broadcast", text: "赤霄纪已至。", style: "era_decree" }],
            reasoning: "era",
          },
        },
      ],
      metadata: { sourceTick: state.tick, correlationId: `tiandao-tick-${state.tick}` },
    });

    expect(decisions).toHaveLength(1);
    expect(decisions[0].decision.commands[0]).toMatchObject({
      type: "faction_event",
      target: "defend",
      params: {
        kind: "adjust_loyalty_bias",
        faction_id: "defend",
        loyalty_delta: 0.05,
      },
    });
  });

  it("enqueues interception mission for tribulation target with hostile disciples", () => {
    const state = createTestWorldState();
    state.tick = 3_000;
    state.players[0] = {
      ...state.players[0],
      uuid: "offline:rogue-cultivator",
      name: "RogueCultivator",
      zone: "starter_zone",
      breakdown: { ...state.players[0].breakdown, karma: 0.6 },
    };
    state.zones[0] = { ...state.zones[0], active_events: ["du_xu_tribulation"] };
    state.npcs = [
      {
        id: "npc_1v1",
        kind: "minecraft:player",
        pos: [4, 64, 4],
        state: "idle",
        blackboard: {},
        digest: {
          archetype: "disciple",
          age_band: "adult",
          age_ratio: 0.2,
          disciple: {
            faction_id: "attack",
            rank: "disciple",
            loyalty: 0.8,
          },
        },
      },
    ];

    const decisions = produce(state);
    expect(decisions).toHaveLength(1);
    expect(decisions[0].decision.commands[0]).toMatchObject({
      type: "faction_event",
      target: "attack",
      params: expect.objectContaining({
        kind: "enqueue_mission",
        faction_id: "attack",
        subject_id: "offline:rogue-cultivator",
      }),
    });
    expect(decisions[0].decision.commands[0].params["mission_id"]).toBe(
      "mission:intercept_duxu:3000:offline:rogue-cultivator",
    );
  });

  it("emits only commands accepted by the shared agent-command contract", () => {
    const state = createTestWorldState();
    state.tick = 1_200;
    state.zones = [zone("green_cloud_peak", 0.86)];

    for (const produced of produce(state)) {
      for (const command of produced.decision.commands) {
        const result = validateAgentCommandV1Contract({
          v: 1,
          id: "cmd_npc_producer_test",
          source: "arbiter",
          commands: [command],
        });
        expect(result.ok).toBe(true);
      }
    }
  });
});
