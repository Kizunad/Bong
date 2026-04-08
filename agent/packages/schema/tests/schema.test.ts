import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { TSchema } from "@sinclair/typebox";
import { describe, expect, it } from "vitest";

import { AgentCommandV1 } from "../src/agent-command.js";
import { ChatMessageV1 } from "../src/chat-message.js";
import {
  INTENSITY_MAX,
  INTENSITY_MIN,
  MAX_COMMANDS_PER_TICK,
  MAX_NARRATION_LENGTH,
  NEWBIE_POWER_THRESHOLD,
  SPIRIT_QI_TOTAL,
} from "../src/common.js";
import {
  AgentCommandV1 as RootAgentCommandV1,
  ChatMessageV1 as RootChatMessageV1,
  NarrationV1 as RootNarrationV1,
  WorldStateV1 as RootWorldStateV1,
  validate as rootValidate,
} from "../src/index.js";
import { NarrationV1 } from "../src/narration.js";
import { validate } from "../src/validate.js";
import { WorldStateV1 } from "../src/world-state.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

type JsonObject = Record<string, unknown>;

function loadSample<T = unknown>(name: string): T {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf-8")) as T;
}

function loadSampleObject(name: string): JsonObject {
  return loadSample<JsonObject>(name);
}

function expectObject(value: unknown): JsonObject {
  expect(typeof value).toBe("object");
  expect(value).not.toBeNull();
  expect(Array.isArray(value)).toBe(false);

  return value as JsonObject;
}

function expectObjectArray(value: unknown): JsonObject[] {
  expect(Array.isArray(value)).toBe(true);

  return value as JsonObject[];
}

function expectFirstObject(value: unknown): JsonObject {
  const items = expectObjectArray(value);
  const [firstItem] = items;

  expect(firstItem).toBeDefined();

  return expectObject(firstItem);
}

function expectInvalid(schema: TSchema, data: unknown): void {
  const result = rootValidate(schema, data);

  expect(result.ok, result.errors.join("; ")).toBe(false);
  expect(result.errors.length).toBeGreaterThan(0);
}

// ─── Sample validation ─────────────────────────────────

describe("sample files pass schema validation", () => {
  it("world-state.sample.json", () => {
    const data = loadSample("world-state.sample.json");
    const result = rootValidate(RootWorldStateV1, data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("agent-command.sample.json", () => {
    const data = loadSample("agent-command.sample.json");
    const result = rootValidate(RootAgentCommandV1, data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("narration.sample.json", () => {
    const data = loadSample("narration.sample.json");
    const result = rootValidate(RootNarrationV1, data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("chat-message.sample.json", () => {
    const data = loadSample("chat-message.sample.json");
    const result = rootValidate(RootChatMessageV1, data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });
});

describe("package root exports runtime validation helpers", () => {
  it("re-exports validate and shared schemas from index", () => {
    const data = loadSample("world-state.sample.json");
    const result = validate(WorldStateV1, data);

    expect(rootValidate).toBe(validate);
    expect(RootWorldStateV1).toBe(WorldStateV1);
    expect(RootAgentCommandV1).toBe(AgentCommandV1);
    expect(RootNarrationV1).toBe(NarrationV1);
    expect(RootChatMessageV1).toBe(ChatMessageV1);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });
});

// ─── Rejection tests ───────────────────────────────────

describe("schema rejects invalid data", () => {
  it("rejects world state with wrong version", () => {
    const data = loadSampleObject("world-state.sample.json");

    data.v = 2;

    expectInvalid(WorldStateV1, data);
  });

  it("rejects world state missing players", () => {
    const data = loadSampleObject("world-state.sample.json");

    delete data.players;

    expectInvalid(WorldStateV1, data);
  });

  it("rejects world state with invalid player trend enum", () => {
    const data = loadSampleObject("world-state.sample.json");
    const firstPlayer = expectFirstObject(data.players);

    firstPlayer.trend = "ascending";

    expectInvalid(WorldStateV1, data);
  });

  it("rejects command batch with wrong version", () => {
    const data = loadSampleObject("agent-command.sample.json");

    data.v = 2;

    expectInvalid(AgentCommandV1, data);
  });

  it("rejects command batch missing id", () => {
    const data = loadSampleObject("agent-command.sample.json");

    delete data.id;

    expectInvalid(AgentCommandV1, data);
  });

  it("rejects command with invalid type", () => {
    const data = {
      v: 1,
      id: "cmd_test",
      commands: [{ type: "delete_world", target: "everywhere", params: {} }],
    };

    expectInvalid(AgentCommandV1, data);
  });

  it("rejects command batch exceeding max commands per tick", () => {
    const data = {
      v: 1,
      id: "cmd_over_limit",
      commands: Array.from({ length: MAX_COMMANDS_PER_TICK + 1 }, (_, index) => ({
        type: "spawn_event",
        target: `zone_${index}`,
        params: {},
      })),
    };

    expectInvalid(AgentCommandV1, data);
  });

  it("rejects narration batch with wrong version", () => {
    const data = loadSampleObject("narration.sample.json");

    data.v = 2;

    expectInvalid(NarrationV1, data);
  });

  it("rejects narration without text", () => {
    const data = {
      v: 1,
      narrations: [{ scope: "broadcast", style: "system_warning" }],
    };

    expectInvalid(NarrationV1, data);
  });

  it("rejects narration with invalid style enum", () => {
    const data = loadSampleObject("narration.sample.json");
    const firstNarration = expectFirstObject(data.narrations);

    firstNarration.style = "oracle";

    expectInvalid(NarrationV1, data);
  });

  it("rejects narration exceeding max text length", () => {
    const data = loadSampleObject("narration.sample.json");
    const firstNarration = expectFirstObject(data.narrations);

    firstNarration.text = "天".repeat(MAX_NARRATION_LENGTH + 1);

    expectInvalid(NarrationV1, data);
  });

  it("rejects chat message with wrong version", () => {
    const data = loadSampleObject("chat-message.sample.json");

    data.v = 99;

    expectInvalid(ChatMessageV1, data);
  });

  it("rejects chat message missing player", () => {
    const data = loadSampleObject("chat-message.sample.json");

    delete data.player;

    expectInvalid(ChatMessageV1, data);
  });

  it("rejects chat message exceeding raw length", () => {
    const data = loadSampleObject("chat-message.sample.json");

    data.raw = "a".repeat(257);

    expectInvalid(ChatMessageV1, data);
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
