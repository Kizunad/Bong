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
  });

  it("falls back to an empty decision when JSON is invalid", () => {
    const decision = parseDecision("not json at all");

    expect(decision).toEqual({
      commands: [],
      narrations: [],
      reasoning: "no action",
    });
  });
});
