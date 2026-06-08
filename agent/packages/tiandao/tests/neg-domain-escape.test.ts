import { describe, expect, it } from "vitest";
import type { Command, WorldStateV1 } from "@bong/schema";
import { Arbiter } from "../src/arbiter.js";
import { applyNegDomainTribulationGate, renderNegDomainNarrations } from "../src/neg-domain-escape.js";
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

function cultivationSnapshot(args: { realm: "Awaken" | "Induce" | "Condense" | "Solidify" | "Spirit" | "Void"; qiCurrent: number; qiMax: number }) {
  return {
    realm: args.realm,
    qi_current: args.qiCurrent,
    qi_max: args.qiMax,
    qi_max_frozen: args.qiMax,
    meridians_opened: 12,
    meridians_total: 20,
    qi_color_main: "Sharp" as const,
    qi_color_chaotic: false,
    qi_color_hunyuan: false,
    composure: 0.8,
  };
}

function previousAndCurrentForDrowning(args: {
  previousQi: number;
  currentQi: number;
  victimRealm?: "Condense" | "Solidify" | "Spirit" | "Void";
  baiterRealm?: "Awaken" | "Induce" | "Condense" | "Solidify";
  tickDelta?: number;
}): { previousState: WorldStateV1; state: WorldStateV1 } {
  const previousState = stateWithPlayer({ realm: args.victimRealm ?? "Spirit", spiritQi: -0.3, tick: 100 });
  previousState.players[0] = {
    ...previousState.players[0],
    uuid: "victim",
    name: "高修乙",
    cultivation: cultivationSnapshot({ realm: args.victimRealm ?? "Spirit", qiCurrent: args.previousQi, qiMax: 100 }),
  };
  previousState.players.push({
    ...previousState.players[0],
    uuid: "baiter",
    name: "低修甲",
    realm: args.baiterRealm ?? "Condense",
    cultivation: cultivationSnapshot({ realm: args.baiterRealm ?? "Condense", qiCurrent: 20, qiMax: 30 }),
  });

  const state = stateWithPlayer({ realm: args.victimRealm ?? "Spirit", spiritQi: -0.3, tick: 100 + (args.tickDelta ?? 5) });
  state.players = previousState.players.map((player) => ({ ...player }));
  state.players[0] = {
    ...state.players[0],
    cultivation: cultivationSnapshot({ realm: args.victimRealm ?? "Spirit", qiCurrent: args.currentQi, qiMax: 100 }),
  };
  return { previousState, state };
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

describe("plan-neg-domain-escape-v1 P1 — 负灵域叙事正典化", () => {
  it("broadcasts spirit-realm lock loss when player enters negative domain", () => {
    const previousState = stateWithPlayer({ realm: "Spirit", spiritQi: 0.2, playerZone: "safe_zone", tick: 10 });
    const state = stateWithPlayer({ realm: "Spirit", spiritQi: -0.2, playerZone: "negative_domain", tick: 11 });

    const narrations = renderNegDomainNarrations({ previousState, state });

    expect(narrations).toEqual([
      expect.objectContaining({ scope: "broadcast", target: "player-spirit", style: "system_warning" }),
    ]);
    expect(narrations[0]?.text).toContain("天道视线");
  });

  it("broadcasts relock when spirit-realm player leaves negative domain", () => {
    const previousState = stateWithPlayer({ realm: "Spirit", spiritQi: -0.2, playerZone: "negative_domain", tick: 20 });
    const state = stateWithPlayer({ realm: "Spirit", spiritQi: 0.2, playerZone: "safe_zone", tick: 21 });

    const narrations = renderNegDomainNarrations({ previousState, state });

    expect(narrations[0]).toEqual(
      expect.objectContaining({ scope: "broadcast", target: "player-spirit", style: "system_warning" }),
    );
    expect(narrations[0]?.text).toContain("重新合拢");
  });

  it("emits private baiter hint and zone narration for large qi drop in five ticks", () => {
    const { previousState, state } = previousAndCurrentForDrowning({ previousQi: 90, currentQi: 60 });

    const narrations = renderNegDomainNarrations({ previousState, state });

    expect(narrations).toEqual([
      expect.objectContaining({ scope: "player", target: "baiter", style: "perception" }),
      expect.objectContaining({ scope: "zone", target: "negative_domain", style: "narration" }),
    ]);
    expect(narrations[0]?.text).toContain("这是你的机会");
    expect(narrations[1]?.text).toContain("不偏向强者");
  });

  it("does not emit drowning hint when realm gap is less than two", () => {
    const { previousState, state } = previousAndCurrentForDrowning({
      previousQi: 90,
      currentQi: 60,
      victimRealm: "Spirit",
      baiterRealm: "Solidify",
    });

    expect(renderNegDomainNarrations({ previousState, state })).toEqual([]);
  });

  it("does not emit drowning hint below the 25 percent qi_max drop threshold", () => {
    const { previousState, state } = previousAndCurrentForDrowning({ previousQi: 90, currentQi: 66 });

    expect(renderNegDomainNarrations({ previousState, state })).toEqual([]);
  });

  it("does not emit drowning hint outside the five tick window", () => {
    const { previousState, state } = previousAndCurrentForDrowning({ previousQi: 90, currentQi: 60, tickDelta: 6 });

    expect(renderNegDomainNarrations({ previousState, state })).toEqual([]);
  });
});
