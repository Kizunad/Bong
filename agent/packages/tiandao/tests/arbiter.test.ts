import { describe, expect, it } from "vitest";
import { MAX_COMMANDS_PER_TICK } from "@bong/schema";
import { Arbiter, type SourcedDecision } from "../src/arbiter.js";
import { createTestWorldState } from "./support/fakes.js";
import type { WorldStateV1 } from "@bong/schema";

function runMerge(decisions: SourcedDecision[], state: WorldStateV1 = createTestWorldState()) {
  return new Arbiter(state).merge(decisions);
}

describe("Arbiter", () => {
  it("merges same-zone modify_zone commands", () => {
    const state = createTestWorldState();
    state.zones.push({
      name: "second_zone",
      spirit_qi: 0.5,
      danger_level: 1,
      active_events: [],
      player_count: 0,
    });

    const result = runMerge([
      {
        source: "mutation",
        decision: {
          commands: [
            {
              type: "modify_zone",
              target: "starter_zone",
              params: { spirit_qi_delta: 0.2, danger_level_delta: 1 },
            },
            {
              type: "modify_zone",
              target: "second_zone",
              params: { spirit_qi_delta: -0.2 },
            },
          ],
          narrations: [],
          reasoning: "mutation",
        },
      },
      {
        source: "calamity",
        decision: {
          commands: [
            {
              type: "modify_zone",
              target: "starter_zone",
              params: { spirit_qi_delta: -0.05, danger_level_delta: -2 },
            },
            {
              type: "modify_zone",
              target: "second_zone",
              params: { spirit_qi_delta: 0.05 },
            },
          ],
          narrations: [],
          reasoning: "calamity",
        },
      },
    ], state);

    const starterMerge = result.commands.find(
      (command) => command.type === "modify_zone" && command.target === "starter_zone",
    );
    expect(starterMerge).toBeDefined();
    expect(starterMerge?.params["spirit_qi_delta"]).toBeCloseTo(0.15, 6);
    expect(starterMerge?.params["danger_level_delta"]).toBeCloseTo(-1, 6);
  });

  it("applies spirit-qi conservation scaling", () => {
    const state = createTestWorldState();
    state.zones.push({
      name: "second_zone",
      spirit_qi: 0.5,
      danger_level: 1,
      active_events: [],
      player_count: 0,
    });

    const result = runMerge([
      {
        source: "mutation",
        decision: {
          commands: [
            {
              type: "modify_zone",
              target: "starter_zone",
              params: { spirit_qi_delta: 0.4 },
            },
          ],
          narrations: [],
          reasoning: "up",
        },
      },
      {
        source: "calamity",
        decision: {
          commands: [
            {
              type: "modify_zone",
              target: "second_zone",
              params: { spirit_qi_delta: -0.1 },
            },
          ],
          narrations: [],
          reasoning: "down",
        },
      },
    ], state);

    expect(result.commands).toHaveLength(2);
    const modifyCommands = result.commands.filter((command) => command.type === "modify_zone");
    expect(modifyCommands).toHaveLength(2);

    const totalSpiritQiDelta = modifyCommands.reduce((sum, command) => {
      const delta = command.params["spirit_qi_delta"];
      return sum + (typeof delta === "number" ? delta : 0);
    }, 0);

    expect(totalSpiritQiDelta).toBeCloseTo(0, 6);
  });

  it("drops hard-constraint violations", () => {
    const state = createTestWorldState();
    state.players.push({
      uuid: "offline:newbie",
      name: "newbie",
      realm: "qi_refining_1",
      composite_power: 0.1,
      breakdown: {
        combat: 0.1,
        wealth: 0.1,
        social: 0.1,
        karma: 0,
        territory: 0.1,
      },
      trend: "stable",
      active_hours: 1,
      zone: "starter_zone",
      pos: [1, 64, 1],
      recent_kills: 0,
      recent_deaths: 0,
    });

    const result = runMerge([
      {
        source: "mutation",
        decision: {
          commands: [
            {
              type: "spawn_event",
              target: "starter_zone",
              params: { event: "thunder_tribulation", intensity: 2 },
            },
            {
              type: "spawn_event",
              target: "starter_zone",
              params: { event: "beast_tide", intensity: 0.5, target_player: "offline:newbie" },
            },
            {
              type: "modify_zone",
              target: "unknown_zone",
              params: { spirit_qi_delta: 0.1 },
            },
            {
              type: "modify_zone",
              target: "starter_zone",
              params: { spirit_qi_delta: 0.05 },
            },
          ],
          narrations: [],
          reasoning: "constraints",
        },
      },
    ], state);

    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].type).toBe("modify_zone");
    expect(result.commands[0].target).toBe("starter_zone");
  });

  it("resolves spawn_event priority as Era > Mutation > Calamity", () => {
    const makeSpawn = (event: string) => ({
      type: "spawn_event" as const,
      target: "starter_zone",
      params: { event, intensity: 0.5 },
    });

    const result = runMerge([
      {
        source: "calamity",
        decision: {
          commands: [makeSpawn("thunder_tribulation")],
          narrations: [],
          reasoning: "c",
        },
      },
      {
        source: "Mutation",
        decision: {
          commands: [makeSpawn("beast_tide")],
          narrations: [],
          reasoning: "m",
        },
      },
      {
        source: "Era",
        decision: {
          commands: [makeSpawn("karma_backlash")],
          narrations: [],
          reasoning: "e",
        },
      },
    ]);

    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].type).toBe("spawn_event");
    expect(result.commands[0].params["event"]).toBe("karma_backlash");
  });

  it("truncates merged commands to MAX_COMMANDS_PER_TICK", () => {
    const state = createTestWorldState();
    for (let i = 0; i < MAX_COMMANDS_PER_TICK + 2; i++) {
      state.zones.push({
        name: `zone_${i}`,
        spirit_qi: 0.5,
        danger_level: 1,
        active_events: [],
        player_count: 0,
      });
    }

    const commands = Array.from({ length: MAX_COMMANDS_PER_TICK + 2 }, (_, i) => ({
      type: "spawn_event" as const,
      target: `zone_${i}`,
      params: {
        event: `event_${i}`,
        intensity: 0.1,
      },
    }));

    const result = runMerge([
      {
        source: "era",
        decision: {
          commands,
          narrations: [],
          reasoning: "truncate",
        },
      },
    ], state);

    expect(result.commands).toHaveLength(MAX_COMMANDS_PER_TICK);
  });

  it("keeps both spawn_event and modify_zone for same zone", () => {
    const result = runMerge([
      {
        source: "calamity",
        decision: {
          commands: [
            {
              type: "spawn_event",
              target: "starter_zone",
              params: { event: "thunder_tribulation", intensity: 0.5 },
            },
            {
              type: "modify_zone",
              target: "starter_zone",
              params: { spirit_qi_delta: -0.1 },
            },
          ],
          narrations: [],
          reasoning: "both",
        },
      },
    ]);

    expect(result.commands).toHaveLength(2);
    expect(result.commands.some((command) => command.type === "spawn_event")).toBe(true);
    expect(result.commands.some((command) => command.type === "modify_zone")).toBe(true);
  });

  it("materializes an era decree into currentEra and uniform global modify_zone commands", () => {
    const state = createTestWorldState();
    state.tick = 888;
    state.zones.push({
      name: "green_cloud_peak",
      spirit_qi: 0.8,
      danger_level: 1,
      active_events: [],
      player_count: 0,
    });

    const result = runMerge(
      [
        {
          source: "era",
          decision: {
            commands: [
              {
                type: "modify_zone",
                target: "全局",
                params: {
                  era_name: "末法纪",
                  global_effect: "灵机渐枯，诸域修行更艰",
                  spirit_qi_delta: -0.02,
                  danger_level_delta: 1,
                },
              },
            ],
            narrations: [
              {
                scope: "broadcast",
                text: "天地风色俱沉，旧法如灰，新纪将临。",
                style: "era_decree",
              },
            ],
            reasoning: "declare era",
          },
        },
      ],
      state,
    );

    expect(result.currentEra).toEqual({
      name: "末法纪",
      sinceTick: 888,
      globalEffect: "灵机渐枯，诸域修行更艰",
    });
    expect(result.narrations[0]?.style).toBe("era_decree");

    const modifyCommands = result.commands.filter((command) => command.type === "modify_zone");
    expect(modifyCommands).toHaveLength(2);
    expect(modifyCommands.map((command) => command.target).sort()).toEqual([
      "green_cloud_peak",
      "starter_zone",
    ]);

    for (const command of modifyCommands) {
      expect(command.params["spirit_qi_delta"]).toBeCloseTo(-0.02, 6);
      expect(command.params["danger_level_delta"]).toBe(1);
    }
  });

  it("keeps local conservation while layering era global effects", () => {
    const state = createTestWorldState();
    state.zones.push({
      name: "green_cloud_peak",
      spirit_qi: 0.8,
      danger_level: 1,
      active_events: [],
      player_count: 0,
    });

    const result = runMerge(
      [
        {
          source: "mutation",
          decision: {
            commands: [
              {
                type: "modify_zone",
                target: "starter_zone",
                params: { spirit_qi_delta: 0.04 },
              },
              {
                type: "modify_zone",
                target: "green_cloud_peak",
                params: { spirit_qi_delta: -0.04 },
              },
            ],
            narrations: [],
            reasoning: "local rebalance",
          },
        },
        {
          source: "era",
          decision: {
            commands: [
              {
                type: "modify_zone",
                target: "global",
                params: {
                  era_name: "霜息纪",
                  global_effect: "寒意入脉，诸域同受其律",
                  spirit_qi_delta: -0.01,
                },
              },
            ],
            narrations: [],
            reasoning: "era overlay",
          },
        },
      ],
      state,
    );

    const starter = result.commands.find(
      (command) => command.type === "modify_zone" && command.target === "starter_zone",
    );
    const peak = result.commands.find(
      (command) => command.type === "modify_zone" && command.target === "green_cloud_peak",
    );

    expect(starter?.params["spirit_qi_delta"]).toBeCloseTo(0.03, 6);
    expect(peak?.params["spirit_qi_delta"]).toBeCloseTo(-0.05, 6);
    expect(result.currentEra?.name).toBe("霜息纪");
  });
});
