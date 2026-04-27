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
import { DeathInsightRequestV1 } from "../src/death-insight.js";
import {
  AgingEventV1,
  DeceasedIndexEntryV1,
  DeceasedSnapshotV1,
  DuoSheEventV1,
  LifespanEventV1,
} from "../src/death-lifecycle.js";
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
import {
  TsyNpcSpawnedV1,
  TsySentinelPhaseChangedV1,
} from "../src/tsy-hostile-v1.js";
import {
  SkillCapChangedPayloadV1,
  SkillLvUpPayloadV1,
  SkillSnapshotPayloadV1,
  SkillScrollUsedPayloadV1,
  SkillXpGainPayloadV1,
} from "../src/skill.js";
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

  it("tsy-npc-spawned.sample.json", () => {
    const data = loadSample("tsy-npc-spawned.sample.json");
    const result = validate(TsyNpcSpawnedV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("tsy-sentinel-phase-changed.sample.json", () => {
    const data = loadSample("tsy-sentinel-phase-changed.sample.json");
    const result = validate(TsySentinelPhaseChangedV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.botany-harvest-progress.sample.json", () => {
    const data = loadSample("server-data.botany-harvest-progress.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.botany-skill.sample.json", () => {
    const data = loadSample("server-data.botany-skill.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.cultivation-detail.sample.json", () => {
    const data = loadSample("server-data.cultivation-detail.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.death-screen.sample.json", () => {
    const data = loadSample("server-data.death-screen.sample.json");
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

  it("server-data.skill-xp-gain.sample.json", () => {
    const data = loadSample("server-data.skill-xp-gain.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.skill-lv-up.sample.json", () => {
    const data = loadSample("server-data.skill-lv-up.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.skill-cap-changed.sample.json", () => {
    const data = loadSample("server-data.skill-cap-changed.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.skill-scroll-used.sample.json", () => {
    const data = loadSample("server-data.skill-scroll-used.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.skill-snapshot.sample.json", () => {
    const data = loadSample("server-data.skill-snapshot.sample.json");
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

  it("client-request.inventory-move-intent.sample.json", () => {
    const data = loadSample("client-request.inventory-move-intent.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.apply-pill.sample.json", () => {
    const data = loadSample("client-request.apply-pill.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.duo-she-request.sample.json", () => {
    const data = loadSample("client-request.duo-she-request.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.use-life-core.sample.json", () => {
    const data = loadSample("client-request.use-life-core.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.pickup-dropped-item.sample.json", () => {
    const data = loadSample("client-request.pickup-dropped-item.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.mineral-probe.sample.json", () => {
    const data = loadSample("client-request.mineral-probe.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.inventory-discard-item.sample.json", () => {
    const data = loadSample("client-request.inventory-discard-item.sample.json");
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

  it("death-insight-request.sample.json", () => {
    const data = loadSample("death-insight-request.sample.json");
    const result = validate(DeathInsightRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("deceased-index-entry.sample.json", () => {
    const data = loadSample("deceased-index-entry.sample.json");
    const result = validate(DeceasedIndexEntryV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("deceased-snapshot.sample.json", () => {
    const data = loadSample("deceased-snapshot.sample.json");
    const result = validate(DeceasedSnapshotV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("lifespan-event.sample.json", () => {
    const data = loadSample("lifespan-event.sample.json");
    const result = validate(LifespanEventV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("aging-event.sample.json", () => {
    const data = loadSample("aging-event.sample.json");
    const result = validate(AgingEventV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("duo-she-event.sample.json", () => {
    const data = loadSample("duo-she-event.sample.json");
    const result = validate(DuoSheEventV1, data);
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

// plan-skill-v1 §8 IPC schema — 4 份 sample 均为"多案例数组"，每条都要过 validate。
describe("skill IPC payload samples pass schema validation", () => {
  function expectAllPass<S extends Parameters<typeof validate>[0]>(
    sampleFile: string,
    schema: S,
  ): void {
    const arr = loadSample(sampleFile);
    expect(Array.isArray(arr), `${sampleFile} must be a JSON array`).toBe(true);
    for (const [i, entry] of (arr as unknown[]).entries()) {
      const result = validate(schema, entry);
      expect(
        result.ok,
        `${sampleFile}[${i}] should pass: ${result.errors.join("; ")}`,
      ).toBe(true);
    }
  }

  it("skill-xp-gain.sample.json", () => {
    expectAllPass("skill-xp-gain.sample.json", SkillXpGainPayloadV1);
  });

  it("skill-lv-up.sample.json", () => {
    expectAllPass("skill-lv-up.sample.json", SkillLvUpPayloadV1);
  });

  it("skill-cap-changed.sample.json", () => {
    expectAllPass("skill-cap-changed.sample.json", SkillCapChangedPayloadV1);
  });

  it("skill-scroll-used.sample.json", () => {
    expectAllPass(
      "skill-scroll-used.sample.json",
      SkillScrollUsedPayloadV1,
    );
  });

  it("skill-snapshot.sample.json", () => {
    expectAllPass("skill-snapshot.sample.json", SkillSnapshotPayloadV1);
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

  it("accepts life record skill milestone snapshots in world state", () => {
    const data = loadObjectSample("world-state.sample.json");
    const firstPlayer = (data.players as Array<Record<string, unknown>>)[0];
    const lifeRecord = firstPlayer.life_record as Record<string, unknown>;
    expect(Array.isArray(lifeRecord.skill_milestones)).toBe(true);
    expect((lifeRecord.skill_milestones as unknown[]).length).toBe(2);

    expectContractAccepts(
      "WorldStateV1 life record skill milestone snapshots",
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
