import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { WorldStateV1 } from "../src/world-state.js";
import { AgentCommandV1 } from "../src/agent-command.js";
import { NarrationV1 } from "../src/narration.js";
import { ChatMessageV1 } from "../src/chat-message.js";
import { validate } from "../src/validate.js";
import {
  INTENSITY_MAX,
  INTENSITY_MIN,
  MAX_COMMANDS_PER_TICK,
  MAX_NARRATION_LENGTH,
  NEWBIE_POWER_THRESHOLD,
  SPIRIT_QI_TOTAL,
} from "../src/common.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

function loadSample(name: string): unknown {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf-8"));
}

// ─── Sample validation ─────────────────────────────────

describe("sample files pass schema validation", () => {
  it("world-state.sample.json", () => {
    const data = loadSample("world-state.sample.json");
    const result = validate(WorldStateV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("agent-command.sample.json", () => {
    const data = loadSample("agent-command.sample.json");
    const result = validate(AgentCommandV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("narration.sample.json", () => {
    const data = loadSample("narration.sample.json");
    const result = validate(NarrationV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("chat-message.sample.json", () => {
    const data = loadSample("chat-message.sample.json");
    const result = validate(ChatMessageV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });
});

// ─── Rejection tests ───────────────────────────────────

describe("schema rejects invalid data", () => {
  it("rejects world state with wrong version", () => {
    const data = loadSample("world-state.sample.json") as any;
    data.v = 2;
    const result = validate(WorldStateV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects world state missing players", () => {
    const data = loadSample("world-state.sample.json") as any;
    delete data.players;
    const result = validate(WorldStateV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects command with invalid type", () => {
    const data = {
      v: 1,
      id: "cmd_test",
      commands: [{ type: "delete_world", target: "everywhere", params: {} }],
    };
    const result = validate(AgentCommandV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects narration without text", () => {
    const data = {
      v: 1,
      narrations: [{ scope: "broadcast", style: "system_warning" }],
    };
    const result = validate(NarrationV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects chat message with wrong version", () => {
    const data = loadSample("chat-message.sample.json") as any;
    data.v = 99;
    const result = validate(ChatMessageV1, data);
    expect(result.ok).toBe(false);
  });
});

// ─── Constants sanity ──────────────────────────────────

describe("shared constants are sane", () => {
  it("intensity range is [0, 1]", () => {
    expect(INTENSITY_MIN).toBe(0.0);
    expect(INTENSITY_MAX).toBe(1.0);
  });

  it("spirit qi total is 100", () => {
    expect(SPIRIT_QI_TOTAL).toBe(100.0);
  });

  it("max commands per tick is 5", () => {
    expect(MAX_COMMANDS_PER_TICK).toBe(5);
  });

  it("newbie threshold is 0.2", () => {
    expect(NEWBIE_POWER_THRESHOLD).toBe(0.2);
  });

  it("max narration length is 500", () => {
    expect(MAX_NARRATION_LENGTH).toBe(500);
  });
});
