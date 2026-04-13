import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";

import {
  AgentCommandV1,
  validateAgentCommandV1Contract,
} from "../src/agent-command.js";
import { ChatMessageV1 } from "../src/chat-message.js";
import { CHANNELS, REDIS_V1_CHANNELS } from "../src/channels.js";
import { CombatRealtimeEventV1, CombatSummaryV1 } from "../src/combat-event.js";
import {
  INTENSITY_MAX,
  INTENSITY_MIN,
  MAX_COMMANDS_PER_TICK,
  MAX_NARRATION_LENGTH,
  NEWBIE_POWER_THRESHOLD,
  SPIRIT_QI_TOTAL,
} from "../src/common.js";
import * as SchemaPackage from "../src/index.js";
import {
  NarrationV1,
  validateNarrationV1Contract,
} from "../src/narration.js";
import { ServerDataV1 } from "../src/server-data.js";
import { validate } from "../src/validate.js";
import {
  WorldStateV1,
  validateWorldStateV1Contract,
} from "../src/world-state.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

function loadSample(name: string): unknown {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf-8"));
}

function loadObjectSample(name: string): Record<string, unknown> {
  const sample = loadSample(name);
  expect(typeof sample).toBe("object");
  expect(sample).not.toBeNull();
  return sample as Record<string, unknown>;
}

function asObject(value: unknown): Record<string, unknown> {
  expect(typeof value).toBe("object");
  expect(value).not.toBeNull();
  return value as Record<string, unknown>;
}

function asArray(value: unknown): unknown[] {
  expect(Array.isArray(value)).toBe(true);
  return value as unknown[];
}

type ContractValidation = (data: unknown) => { ok: boolean; errors: string[] };

function expectContractAccepts(name: string, validator: ContractValidation, data: unknown): void {
  const result = validator(data);
  expect(result.ok, `${name} should be accepted: ${result.errors.join("; ")}`).toBe(true);
}

function expectContractRejects(name: string, validator: ContractValidation, data: unknown): void {
  const result = validator(data);
  expect(result.ok, `${name} should be rejected`).toBe(false);
}

function loadPackageJson(): {
  exports?: Record<string, unknown>;
} {
  return JSON.parse(readFileSync(join(__dirname, "..", "package.json"), "utf-8")) as {
    exports?: Record<string, unknown>;
  };
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

  it("server-data.welcome.sample.json", () => {
    const data = loadSample("server-data.welcome.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.heartbeat.sample.json", () => {
    const data = loadSample("server-data.heartbeat.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.narration.sample.json", () => {
    const data = loadSample("server-data.narration.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.zone-info.sample.json", () => {
    const data = loadSample("server-data.zone-info.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.event-alert.sample.json", () => {
    const data = loadSample("server-data.event-alert.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.player-state.sample.json", () => {
    const data = loadSample("server-data.player-state.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.ui-open.sample.json", () => {
    const data = loadSample("server-data.ui-open.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.cultivation-detail.sample.json", () => {
    const data = loadSample("server-data.cultivation-detail.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("combat-event.realtime.sample.json", () => {
    const data = loadSample("combat-event.realtime.sample.json");
    const result = validate(CombatRealtimeEventV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("combat-event.summary.sample.json", () => {
    const data = loadSample("combat-event.summary.sample.json");
    const result = validate(CombatSummaryV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });
});

describe("negative sample files fail schema validation", () => {
  it("world-state.invalid-extra-player-field.sample.json", () => {
    const data = loadSample("world-state.invalid-extra-player-field.sample.json");
    const result = validate(WorldStateV1, data);
    expect(result.ok).toBe(false);
  });

  it("agent-command.invalid-extra-command-field.sample.json", () => {
    const data = loadSample("agent-command.invalid-extra-command-field.sample.json");
    const result = validate(AgentCommandV1, data);
    expect(result.ok).toBe(false);
  });

  it("narration.invalid-extra-top-level-field.sample.json", () => {
    const data = loadSample("narration.invalid-extra-top-level-field.sample.json");
    const result = validate(NarrationV1, data);
    expect(result.ok).toBe(false);
  });

  it("chat-message.invalid-extra-top-level-field.sample.json", () => {
    const data = loadSample("chat-message.invalid-extra-top-level-field.sample.json");
    const result = validate(ChatMessageV1, data);
    expect(result.ok).toBe(false);
  });

  it("server-data.invalid-unknown-type.sample.json", () => {
    const data = loadSample("server-data.invalid-unknown-type.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok).toBe(false);
  });
});

// ─── Rejection tests ───────────────────────────────────

describe("schema rejects invalid data", () => {
  it("rejects world state with wrong version", () => {
    const data = loadObjectSample("world-state.sample.json");
    data.v = 2;
    const result = validate(WorldStateV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects world state missing players", () => {
    const data = loadObjectSample("world-state.sample.json");
    delete data.players;
    const result = validate(WorldStateV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects world state with unexpected nested player field", () => {
    const data = loadObjectSample("world-state.sample.json");
    const players = asArray(data.players);
    const firstPlayer = asObject(players[0]);
    firstPlayer.rogue_power = 999;
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

  it("rejects command batch with unexpected top-level field", () => {
    const data = loadObjectSample("agent-command.sample.json");
    data.retry_after_ms = 500;
    const result = validate(AgentCommandV1, data);
    expect(result.ok).toBe(false);
  });

  it("accepts arbiter as agent-command source", () => {
    const data = loadObjectSample("agent-command.sample.json");
    data.source = "arbiter";
    const result = validate(AgentCommandV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("rejects command with unexpected nested field", () => {
    const data = loadObjectSample("agent-command.sample.json");
    const commands = asArray(data.commands);
    const firstCommand = asObject(commands[0]);
    firstCommand.priority = "urgent";
    const result = validate(AgentCommandV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects command batch with more than five commands", () => {
    const data = loadObjectSample("agent-command.sample.json");
    const commands = asArray(data.commands);
    data.commands = [...commands, ...commands, ...commands];
    expectContractRejects(
      "AgentCommandV1.commands maxItems parity gate",
      validateAgentCommandV1Contract,
      data,
    );
  });

  it("rejects command with non-object params", () => {
    const data = loadObjectSample("agent-command.sample.json");
    const commands = asArray(data.commands);
    const firstCommand = asObject(commands[0]);
    firstCommand.params = ["invalid"];
    expectContractRejects(
      "AgentCommandV1.commands[].params object parity gate",
      validateAgentCommandV1Contract,
      data,
    );
  });

  it("rejects narration without text", () => {
    const data = {
      v: 1,
      narrations: [{ scope: "broadcast", style: "system_warning" }],
    };
    const result = validate(NarrationV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects narration batch with unexpected top-level field", () => {
    const data = loadObjectSample("narration.sample.json");
    data.trace_id = "narration-1";
    const result = validate(NarrationV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects narration entry with unexpected nested field", () => {
    const data = loadObjectSample("narration.sample.json");
    const narrations = asArray(data.narrations);
    const firstNarration = asObject(narrations[0]);
    firstNarration.audience = "sect_leaders";
    expectContractRejects(
      "NarrationV1 nested unknown field parity gate",
      validateNarrationV1Contract,
      data,
    );
  });

  it("rejects narration without target when scope is not broadcast", () => {
    const data = loadObjectSample("narration.sample.json");
    const narrations = asArray(data.narrations);
    const targetedNarration = asObject(narrations[1]);
    delete targetedNarration.target;
    expectContractRejects(
      "NarrationV1 scope-target parity gate",
      validateNarrationV1Contract,
      data,
    );
  });

  it("rejects narration with invalid style", () => {
    const data = loadObjectSample("narration.sample.json");
    const narrations = asArray(data.narrations);
    const firstNarration = asObject(narrations[0]);
    firstNarration.style = "ominous_whisper";
    expectContractRejects(
      "NarrationV1.style enum parity gate",
      validateNarrationV1Contract,
      data,
    );
  });

  it("rejects world state with unexpected top-level field", () => {
    const data = loadObjectSample("world-state.sample.json");
    data.realm_clock = 99;
    expectContractRejects(
      "WorldStateV1 top-level unknown field parity gate",
      validateWorldStateV1Contract,
      data,
    );
  });

  it("rejects chat message with wrong version", () => {
    const data = loadObjectSample("chat-message.sample.json");
    data.v = 99;
    const result = validate(ChatMessageV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects chat message with unexpected top-level field", () => {
    const data = loadObjectSample("chat-message.sample.json");
    data.channel = "global";
    const result = validate(ChatMessageV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects server data with unknown payload type", () => {
    const data = loadObjectSample("server-data.welcome.sample.json");
    data.type = "unknown";
    const result = validate(ServerDataV1, data);
    expect(result.ok).toBe(false);
  });

  it("accepts player_state spirit_qi above 100 for breakthrough progression", () => {
    const data = loadObjectSample("server-data.player-state.sample.json");
    data.spirit_qi = 140;
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("accepts Redis V1 contract samples through parity validators", () => {
    expectContractAccepts(
      "WorldStateV1 sample",
      validateWorldStateV1Contract,
      loadSample("world-state.sample.json"),
    );
    expectContractAccepts(
      "AgentCommandV1 sample",
      validateAgentCommandV1Contract,
      loadSample("agent-command.sample.json"),
    );
    expectContractAccepts(
      "NarrationV1 sample",
      validateNarrationV1Contract,
      loadSample("narration.sample.json"),
    );
  });
});

describe("package entrypoints expose runtime validation", () => {
  it("re-exports validate from the root entrypoint", () => {
    expect(SchemaPackage.validate).toBe(validate);
  });

  it("declares a stable validate subpath export", () => {
    const packageJson = loadPackageJson();
    expect(packageJson.exports).toBeDefined();
    expect(packageJson.exports?.["."]).toBeDefined();
    expect(packageJson.exports?.["./validate"]).toBeDefined();
  });
});

// ─── Constants sanity ──────────────────────────────────

describe("shared constants are sane", () => {
  it("Redis V1 channel constants remain frozen", () => {
    expect(CHANNELS).toEqual({
      WORLD_STATE: "bong:world_state",
      PLAYER_CHAT: "bong:player_chat",
      AGENT_COMMAND: "bong:agent_command",
      AGENT_NARRATE: "bong:agent_narrate",
      INSIGHT_REQUEST: "bong:insight_request",
      INSIGHT_OFFER: "bong:insight_offer",
      BREAKTHROUGH_EVENT: "bong:breakthrough_event",
      FORGE_EVENT: "bong:forge_event",
      CULTIVATION_DEATH: "bong:cultivation_death",
      COMBAT_REALTIME: "bong:combat_realtime",
      COMBAT_SUMMARY: "bong:combat_summary",
    });
    expect(REDIS_V1_CHANNELS).toEqual([
      "bong:world_state",
      "bong:player_chat",
      "bong:agent_command",
      "bong:agent_narrate",
      "bong:insight_request",
      "bong:insight_offer",
      "bong:breakthrough_event",
      "bong:forge_event",
      "bong:cultivation_death",
      "bong:combat_realtime",
      "bong:combat_summary",
    ]);
  });

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
