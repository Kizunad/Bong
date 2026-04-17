import { describe, expect, it } from "vitest";
import { parseDecision } from "../src/parse.js";

describe("parseDecision", () => {
  it("extracts JSON from fenced blocks and preserves structured fields", () => {
    const decision = parseDecision(
      "```json\n" +
        "{\n" +
        '  "commands": [\n' +
        '    { "type": "modify_zone", "target": "全局", "params": { "era_name": "末法纪", "global_effect": "灵机渐枯", "spirit_qi_delta": -0.02 } }\n' +
        "  ],\n" +
        '  "narrations": [\n' +
        '    { "scope": "broadcast", "text": "天地沉沉，旧脉将歇。", "style": "era_decree" }\n' +
        "  ],\n" +
        '  "reasoning": "era"\n' +
        "}\n" +
        "```",
    );

    expect(decision.commands).toHaveLength(1);
    expect(decision.commands[0]?.target).toBe("全局");
    expect(decision.commands[0]?.params.era_name).toBe("末法纪");
    expect(decision.commands[0]?.params.global_effect).toBe("灵机渐枯");
    expect(decision.narrations[0]?.style).toBe("era_decree");
    expect(decision.reasoning).toBe("era");
    expect(decision.parseFailures).toBeUndefined();
  });

  it("falls back to an empty decision when JSON is invalid", () => {
    const decision = parseDecision("not json at all");

    expect(decision).toEqual({
      commands: [],
      narrations: [],
      reasoning: "no action",
    });
  });

  it("drops invalid command and narration rows while keeping valid ones and counting failures", () => {
    const decision = parseDecision(
      JSON.stringify({
        commands: [
          { type: "modify_zone", target: "starter_zone", params: { spirit_qi_delta: 0.1 } },
          { type: "bad_command", target: "starter_zone", params: {} },
          { type: "spawn_event", params: { event: "beast_tide" } },
        ],
        narrations: [
          { scope: "broadcast", text: "天地异动", style: "system_warning" },
          { scope: "zone", text: "缺少 target", style: "narration" },
          { scope: "broadcast", text: 42, style: "narration" },
        ],
        reasoning: "mixed validity",
      }),
    );

    expect(decision.commands).toEqual([
      {
        type: "modify_zone",
        target: "starter_zone",
        params: { spirit_qi_delta: 0.1 },
      },
    ]);
    expect(decision.narrations).toEqual([
      {
        scope: "broadcast",
        text: "天地异动",
        style: "system_warning",
      },
    ]);
    expect(decision.reasoning).toBe("mixed validity");
    expect(decision.parseFailures).toEqual({
      commands: 2,
      narrations: 2,
      total: 4,
    });
  });

  it("keeps the first five valid commands in order even when invalid rows are mixed in", () => {
    const decision = parseDecision(
      JSON.stringify({
        commands: [
          { type: "modify_zone", target: "z1", params: { spirit_qi_delta: 0.1 } },
          { type: "invalid", target: "zx", params: {} },
          { type: "modify_zone", target: "z2", params: { spirit_qi_delta: 0.2 } },
          { type: "modify_zone", target: "z3", params: { spirit_qi_delta: 0.3 } },
          { type: "modify_zone", target: "z4", params: { spirit_qi_delta: 0.4 } },
          { type: "modify_zone", target: "z5", params: { spirit_qi_delta: 0.5 } },
          { type: "modify_zone", target: "z6", params: { spirit_qi_delta: 0.6 } },
        ],
        narrations: [],
        reasoning: "keep first five valid commands",
      }),
    );

    expect(decision.commands.map((command) => command.target)).toEqual(["z1", "z2", "z3", "z4", "z5"]);
    expect(decision.parseFailures).toEqual({
      commands: 1,
      narrations: 0,
      total: 1,
    });
  });

  it("keeps valid spawn_npc commands after schema validation", () => {
    const decision = parseDecision(
      JSON.stringify({
        commands: [
          { type: "spawn_npc", target: "spawn", params: { archetype: "zombie" } },
          { type: "spawn_npc", target: "spawn", params: {} },
        ],
        narrations: [],
        reasoning: "spawn one zombie npc",
      }),
    );

    expect(decision.commands).toEqual([
      {
        type: "spawn_npc",
        target: "spawn",
        params: { archetype: "zombie" },
      },
    ]);
    expect(decision.parseFailures).toEqual({
      commands: 1,
      narrations: 0,
      total: 1,
    });
  });

  it("keeps valid faction_event commands after schema validation", () => {
    const decision = parseDecision(
      JSON.stringify({
        commands: [
          {
            type: "faction_event",
            target: "neutral",
            params: {
              kind: "enqueue_mission",
              faction_id: "neutral",
              mission_id: "mission:hold_spawn_gate",
            },
          },
          {
            type: "faction_event",
            target: "neutral",
            params: {
              faction_id: "neutral",
            },
          },
        ],
        narrations: [],
        reasoning: "queue one faction mission",
      }),
    );

    expect(decision.commands).toEqual([
      {
        type: "faction_event",
        target: "neutral",
        params: {
          kind: "enqueue_mission",
          faction_id: "neutral",
          mission_id: "mission:hold_spawn_gate",
        },
      },
    ]);
    expect(decision.parseFailures).toEqual({
      commands: 1,
      narrations: 0,
      total: 1,
    });
  });
});
