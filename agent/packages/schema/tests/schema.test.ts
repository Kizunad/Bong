import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  AgentCommandV1,
  validateAgentCommandV1Contract,
} from "../src/agent-command.js";
import { ChatMessageV1 } from "../src/chat-message.js";
import { CombatRealtimeEventV1, CombatSummaryV1 } from "../src/combat-event.js";
import { InventoryEventV1, InventorySnapshotV1 } from "../src/inventory.js";
import {
  INTENSITY_MAX,
  INTENSITY_MIN,
  MAX_COMMANDS_PER_TICK,
  MAX_NARRATION_LENGTH,
  NEWBIE_POWER_THRESHOLD,
  SPIRIT_QI_TOTAL,
} from "../src/common.js";
import * as SchemaPackage from "../src/index.js";
import { NarrationV1, validateNarrationV1Contract } from "../src/narration.js";
import { ClientRequestV1 } from "../src/client-request.js";
import { ServerDataV1 } from "../src/server-data.js";
import { validate } from "../src/validate.js";
import { VfxEventV1 } from "../src/vfx-event.js";
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

function expectContractAccepts(name: string, validator: ContractValidation, data: unknown): void {
  const result = validator(data);
  expect(result.ok, `${name} should be accepted: ${result.errors.join("; ")}`).toBe(true);
}

function expectContractRejects(name: string, validator: ContractValidation, data: unknown): void {
  const result = validator(data);
  expect(result.ok, `${name} should be rejected`).toBe(false);
}

type ContractValidation = (data: unknown) => { ok: boolean; errors: string[] };

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

  it("server-data.inventory-snapshot.sample.json", () => {
    const data = loadSample("server-data.inventory-snapshot.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.inventory-event.sample.json", () => {
    const data = loadSample("server-data.inventory-event.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.cultivation-detail.sample.json", () => {
    const data = loadSample("server-data.cultivation-detail.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.alchemy-furnace.sample.json", () => {
    const data = loadSample("server-data.alchemy-furnace.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.alchemy-session.sample.json", () => {
    const data = loadSample("server-data.alchemy-session.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.alchemy-outcome-forecast.sample.json", () => {
    const data = loadSample("server-data.alchemy-outcome-forecast.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.alchemy-outcome-resolved.sample.json", () => {
    const data = loadSample("server-data.alchemy-outcome-resolved.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.alchemy-recipe-book.sample.json", () => {
    const data = loadSample("server-data.alchemy-recipe-book.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.alchemy-contamination.sample.json", () => {
    const data = loadSample("server-data.alchemy-contamination.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.alchemy-feed-slot.sample.json", () => {
    const data = loadSample("client-request.alchemy-feed-slot.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.alchemy-ignite.sample.json", () => {
    const data = loadSample("client-request.alchemy-ignite.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.alchemy-intervention.sample.json", () => {
    const data = loadSample("client-request.alchemy-intervention.sample.json");
    const result = validate(ClientRequestV1, data);
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

  it("vfx-event.play-anim.sample.json", () => {
    const data = loadSample("vfx-event.play-anim.sample.json");
    const result = validate(VfxEventV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("vfx-event.stop-anim.sample.json", () => {
    const data = loadSample("vfx-event.stop-anim.sample.json");
    const result = validate(VfxEventV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("vfx-event.spawn-particle.sample.json", () => {
    const data = loadSample("vfx-event.spawn-particle.sample.json");
    const result = validate(VfxEventV1, data);
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

  it("inventory-event.invalid-unknown-kind.sample.json", () => {
    const data = loadSample("inventory-event.invalid-unknown-kind.sample.json");
    const result = validate(InventoryEventV1, data);
    expect(result.ok).toBe(false);
  });

  it("server-data.invalid-unknown-type.sample.json", () => {
    const data = loadSample("server-data.invalid-unknown-type.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok).toBe(false);
  });
});

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

  it("accepts optional faction and disciple summaries", () => {
    const data = loadObjectSample("world-state.sample.json");
    expectContractAccepts(
      "WorldStateV1 optional faction/disciple summaries",
      validateWorldStateV1Contract,
      data,
    );
  });

  it("accepts legacy-compatible world state without faction summaries", () => {
    const data = loadObjectSample("world-state.sample.json");
    delete data.factions;
    const npc = (data.npcs as Array<Record<string, unknown>>)[0];
    const digest = npc.digest as Record<string, unknown>;
    delete digest.disciple;

    expectContractAccepts(
      "WorldStateV1 legacy-compatible optional faction fields",
      validateWorldStateV1Contract,
      data,
    );
  });

  it("accepts arbiter as agent-command source", () => {
    const data = loadObjectSample("agent-command.sample.json");
    data.source = "arbiter";
    const result = validate(AgentCommandV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("accepts spawn_npc command rows", () => {
    const data = loadObjectSample("agent-command.sample.json");
    data.commands = [{ type: "spawn_npc", target: "spawn", params: { archetype: "zombie" } }];
    expectContractAccepts(
      "AgentCommandV1 spawn_npc parity gate",
      validateAgentCommandV1Contract,
      data,
    );
  });

  it("accepts despawn_npc command rows", () => {
    const data = loadObjectSample("agent-command.sample.json");
    data.commands = [{ type: "despawn_npc", target: "npc_2v1", params: {} }];
    expectContractAccepts(
      "AgentCommandV1 despawn_npc parity gate",
      validateAgentCommandV1Contract,
      data,
    );
  });

  it("accepts faction_event command rows", () => {
    const data = loadObjectSample("agent-command.sample.json");
    data.commands = [
      {
        type: "faction_event",
        target: "neutral",
        params: {
          kind: "enqueue_mission",
          faction_id: "neutral",
          mission_id: "mission:hold_spawn_gate",
        },
      },
    ];
    expectContractAccepts(
      "AgentCommandV1 faction_event parity gate",
      validateAgentCommandV1Contract,
      data,
    );
  });

  it("rejects command with more than five commands", () => {
    const data = loadObjectSample("agent-command.sample.json");
    data.commands = [...data.commands, ...data.commands, ...data.commands];
    expectContractRejects(
      "AgentCommandV1.commands maxItems parity gate",
      validateAgentCommandV1Contract,
      data,
    );
  });
});
