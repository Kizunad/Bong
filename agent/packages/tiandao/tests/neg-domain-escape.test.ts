import { describe, expect, it } from "vitest";
import type { Command, WorldStateV1 } from "@bong/schema";
import { Arbiter } from "../src/arbiter.js";
import { applyNegDomainTribulationGate } from "../src/neg-domain-escape.js";
import { WorldModel } from "../src/world-model.js";
import { createTestWorldState } from "./support/fakes.js";

function targetedTribulation(targetPlayer: string): Command {
  return {
    type: "spawn_event",
    target: "negative_domain",
    params: {
      event: "thunder_tribulation",
      target_player: targetPlayer,
      intensity: 0.7,
    },
  };
}

function stateWithPlayer(args: {
  realm: string;
  spiritQi: number;
  playerZone?: string;
  tick?: number;
  compositePower?: number;
}): WorldStateV1 {
  const zone = args.playerZone ?? "negative_domain";
  const state = createTestWorldState();
  state.tick = args.tick ?? 123;
  state.players[0] = {
    ...state.players[0],
    uuid: "player-spirit",
    name: "灵修甲",
    realm: args.realm,
    composite_power: args.compositePower ?? 0.8,
    zone,
  };
  state.zones = [
    {
      name: zone,
      spirit_qi: args.spiritQi,
      danger_level: 3,
      active_events: [],
      player_count: 1,
    },
  ];
  return state;
}

describe("plan-neg-domain-escape-v1 P0 — 负灵域天劫豁免 gate", () => {
  it("suppresses targeted tribulation for Spirit player in negative domain", () => {
    const state = stateWithPlayer({ realm: "Spirit", spiritQi: -0.2 });
    const worldModel = WorldModel.fromState(state);

    const result = applyNegDomainTribulationGate({
      commands: [targetedTribulation("player-spirit")],
      state,
      worldModel,
    });

    expect(result.commands).toEqual([]);
    expect(result.narrations[0]?.text).toContain("负灵域");
    expect(worldModel.getNegDomainPendingTribulation("player-spirit")).toMatchObject({
      playerUuid: "player-spirit",
      zone: "negative_domain",
      enteredAtTick: 123,
      lastSuppressedTick: 123,
    });
  });

  it("allows the same Spirit player after leaving negative domain and clears pending", () => {
    const negativeState = stateWithPlayer({ realm: "Spirit", spiritQi: -0.2, tick: 10 });
    const worldModel = WorldModel.fromState(negativeState);
    applyNegDomainTribulationGate({
      commands: [targetedTribulation("player-spirit")],
      state: negativeState,
      worldModel,
    });

    const positiveState = stateWithPlayer({ realm: "Spirit", spiritQi: 0.1, tick: 11 });
    const result = applyNegDomainTribulationGate({
      commands: [targetedTribulation("player-spirit")],
      state: positiveState,
      worldModel,
    });

    expect(result.commands).toEqual([targetedTribulation("player-spirit")]);
    expect(result.narrations[0]?.text).toContain("重新合拢");
    expect(worldModel.getNegDomainPendingTribulation("player-spirit")).toBeNull();
  });

  it("does not auto-trigger a pending tribulation when LLM emits no command after conditions change", () => {
    const negativeState = stateWithPlayer({ realm: "Spirit", spiritQi: -0.2 });
    const worldModel = WorldModel.fromState(negativeState);
    applyNegDomainTribulationGate({
      commands: [targetedTribulation("player-spirit")],
      state: negativeState,
      worldModel,
    });

    const positiveState = stateWithPlayer({ realm: "Spirit", spiritQi: 0.4, tick: 124 });
    const result = applyNegDomainTribulationGate({
      commands: [],
      state: positiveState,
      worldModel,
    });

    expect(result.commands).toEqual([]);
    expect(result.narrations).toEqual([]);
    expect(worldModel.getNegDomainPendingTribulation("player-spirit")).not.toBeNull();
  });

  it("keeps pending tribulation across WorldModel snapshot restore", () => {
    const state = stateWithPlayer({ realm: "Spirit", spiritQi: -0.3 });
    const worldModel = WorldModel.fromState(state);
    applyNegDomainTribulationGate({
      commands: [targetedTribulation("player-spirit")],
      state,
      worldModel,
    });

    const restored = WorldModel.fromJSON(worldModel.toJSON());

    expect(restored.getNegDomainPendingTribulation("player-spirit")).toMatchObject({
      playerUuid: "player-spirit",
      zone: "negative_domain",
      reason: "negative_domain_tribulation_exempt",
    });
  });

  it("does not suppress Solidify player in negative domain", () => {
    const state = stateWithPlayer({ realm: "Solidify", spiritQi: -0.2 });

    const result = applyNegDomainTribulationGate({
      commands: [targetedTribulation("player-spirit")],
      state,
    });

    expect(result.commands).toEqual([targetedTribulation("player-spirit")]);
  });

  it("does not suppress Condense player in negative domain", () => {
    const state = stateWithPlayer({ realm: "Condense", spiritQi: -0.2 });

    const result = applyNegDomainTribulationGate({
      commands: [targetedTribulation("player-spirit")],
      state,
    });

    expect(result.commands).toEqual([targetedTribulation("player-spirit")]);
  });

  it("does not suppress untargeted zone tribulation", () => {
    const state = stateWithPlayer({ realm: "Spirit", spiritQi: -0.2 });
    const command: Command = {
      type: "spawn_event",
      target: "negative_domain",
      params: {
        event: "thunder_tribulation",
        intensity: 0.7,
      },
    };

    const result = applyNegDomainTribulationGate({ commands: [command], state });

    expect(result.commands).toEqual([command]);
  });

  it("runs after Arbiter merge so LLM cannot bypass the gate", () => {
    const state = stateWithPlayer({ realm: "Spirit", spiritQi: -0.2 });
    const merged = new Arbiter(state).merge([
      {
        source: "calamity",
        decision: {
          commands: [targetedTribulation("player-spirit")],
          narrations: [],
          reasoning: "tries to strike through negative domain",
        },
      },
    ]);

    const result = applyNegDomainTribulationGate({
      commands: merged.commands,
      state,
      worldModel: WorldModel.fromState(state),
    });

    expect(merged.commands).toHaveLength(1);
    expect(result.commands).toHaveLength(0);
  });
});
