import { describe, expect, it } from "vitest";

import { NEWBIE_POWER_THRESHOLD, MAX_COMMANDS_PER_TICK, type WorldStateV1 } from "@bong/schema";
import { Arbiter, MAX_NARRATIONS_PER_TICK } from "../src/arbiter.js";
import { createMockWorldState } from "../src/mock-state.js";
import type { AgentDecision } from "../src/parse.js";

function buildDecision(input: {
  source: "calamity" | "mutation" | "era";
  commands?: Array<{
    type: "spawn_event" | "modify_zone" | "npc_behavior";
    target: string;
    params: Record<string, unknown>;
  }>;
  narrations?: Array<{ scope: "broadcast" | "zone" | "player"; text: string; style: "system_warning" | "perception" | "narration" | "era_decree"; target?: string }>;
}): AgentDecision {
  return {
    commands: (input.commands ?? []).map((cmd) => ({
      type: cmd.type,
      target: cmd.target,
      params: { ...cmd.params },
      _source: input.source,
    })) as AgentDecision["commands"],
    narrations: (input.narrations ?? []) as AgentDecision["narrations"],
    reasoning: `${input.source} decision`,
  };
}

function withNewbieZonePower(state: WorldStateV1, power: number): WorldStateV1 {
  return {
    ...state,
    players: state.players.map((player) =>
      player.zone === "newbie_valley"
        ? {
            ...player,
            composite_power: power,
          }
        : player,
    ),
  };
}

describe("Arbiter.merge", () => {
  it("resolves same-zone spawn conflict by era > mutation > calamity", () => {
    const state = createMockWorldState();
    const arbiter = new Arbiter();

    const merged = arbiter.merge(
      [
        buildDecision({
          source: "calamity",
          commands: [
            {
              type: "spawn_event",
              target: "blood_valley",
              params: { event: "beast_tide", intensity: 0.4 },
            },
          ],
        }),
        buildDecision({
          source: "mutation",
          commands: [
            {
              type: "spawn_event",
              target: "blood_valley",
              params: { event: "karma_backlash", intensity: 0.5 },
            },
          ],
        }),
        buildDecision({
          source: "era",
          commands: [
            {
              type: "spawn_event",
              target: "blood_valley",
              params: { event: "thunder_tribulation", intensity: 0.6 },
            },
          ],
        }),
      ],
      state,
    );

    expect(merged.commands).toHaveLength(1);
    expect(merged.commands[0]).toEqual({
      type: "spawn_event",
      target: "blood_valley",
      params: { event: "thunder_tribulation", intensity: 0.6 },
    });
  });

  it("merges same-zone modify_zone deltas into one command", () => {
    const state = createMockWorldState();
    const arbiter = new Arbiter();

    const merged = arbiter.merge(
      [
        buildDecision({
          source: "calamity",
          commands: [
            {
              type: "modify_zone",
              target: "green_cloud_peak",
              params: { spirit_qi_delta: -0.2, danger_level_delta: 1 },
            },
          ],
        }),
        buildDecision({
          source: "mutation",
          commands: [
            {
              type: "modify_zone",
              target: "green_cloud_peak",
              params: { spirit_qi_delta: 0.19, danger_level_delta: -2 },
            },
          ],
        }),
      ],
      state,
    );

    expect(merged.commands).toHaveLength(1);
    expect(merged.commands[0]?.type).toBe("modify_zone");
    expect(merged.commands[0]?.target).toBe("green_cloud_peak");
    expect(merged.commands[0]?.params.danger_level_delta).toBe(-1);
    expect(merged.commands[0]?.params.spirit_qi_delta as number).toBeCloseTo(-0.01, 6);
  });

  it("scales net spirit_qi_delta toward zero when abs(net) > 0.01", () => {
    const state = createMockWorldState();
    const arbiter = new Arbiter();

    const merged = arbiter.merge(
      [
        buildDecision({
          source: "mutation",
          commands: [
            {
              type: "modify_zone",
              target: "blood_valley",
              params: { spirit_qi_delta: 0.4 },
            },
            {
              type: "modify_zone",
              target: "green_cloud_peak",
              params: { spirit_qi_delta: -0.1 },
            },
          ],
        }),
      ],
      state,
    );

    const net = merged.commands
      .filter((command) => command.type === "modify_zone")
      .reduce((sum, command) => {
        const delta = command.params.spirit_qi_delta;
        return sum + (typeof delta === "number" ? delta : 0);
      }, 0);

    expect(Math.abs(net)).toBeLessThanOrEqual(0.01);
    const firstDelta = merged.commands[0]?.params.spirit_qi_delta;
    const secondDelta = merged.commands[1]?.params.spirit_qi_delta;
    expect(typeof firstDelta).toBe("number");
    expect(typeof secondDelta).toBe("number");
    expect(firstDelta as number).toBeCloseTo(0.1, 6);
    expect(secondDelta as number).toBeCloseTo(-0.1, 6);
  });

  it("blocks high-intensity spawn_event in newbie zone under threshold", () => {
    const state = withNewbieZonePower(createMockWorldState(), NEWBIE_POWER_THRESHOLD - 0.01);
    const arbiter = new Arbiter();

    const merged = arbiter.merge(
      [
        buildDecision({
          source: "calamity",
          commands: [
            {
              type: "spawn_event",
              target: "newbie_valley",
              params: { event: "thunder_tribulation", intensity: 0.8 },
            },
          ],
        }),
      ],
      state,
    );

    expect(merged.commands).toEqual([]);
  });

  it("caps merged command count at MAX_COMMANDS_PER_TICK", () => {
    const state = createMockWorldState();
    const arbiter = new Arbiter();

    const commands = Array.from({ length: MAX_COMMANDS_PER_TICK + 3 }, (_, index) => ({
      type: "npc_behavior" as const,
      target: `npc_${index}`,
      params: { aggression: index / 10 },
    }));

    const merged = arbiter.merge([buildDecision({ source: "era", commands })], state);

    expect(merged.commands).toHaveLength(MAX_COMMANDS_PER_TICK);
  });

  it("preserves narrations but trims to bounded count", () => {
    const state = createMockWorldState();
    const arbiter = new Arbiter();

    const narrations = Array.from({ length: MAX_NARRATIONS_PER_TICK + 5 }, (_, index) => ({
      scope: "broadcast" as const,
      style: "narration" as const,
      text: `narration-${index}`,
    }));

    const merged = arbiter.merge(
      [
        buildDecision({
          source: "mutation",
          narrations,
        }),
      ],
      state,
    );

    expect(merged.narrations).toHaveLength(MAX_NARRATIONS_PER_TICK);
    expect(merged.narrations[0]?.text).toBe("narration-0");
    expect(merged.narrations.at(-1)?.text).toBe(`narration-${MAX_NARRATIONS_PER_TICK - 1}`);
  });

  it("strips private _source from public merged commands", () => {
    const state = createMockWorldState();
    const arbiter = new Arbiter();

    const merged = arbiter.merge(
      [
        buildDecision({
          source: "calamity",
          commands: [
            {
              type: "spawn_event",
              target: "blood_valley",
              params: { event: "beast_tide", intensity: 0.4 },
            },
          ],
        }),
      ],
      state,
    );

    const publicCommand = merged.commands[0] as Record<string, unknown>;
    expect(publicCommand._source).toBeUndefined();
    expect(publicCommand.source).toBeUndefined();
  });
});
