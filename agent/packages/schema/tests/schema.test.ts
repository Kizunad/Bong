import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  AgentCommandV1,
  validateAgentCommandV1Contract,
} from "../src/agent-command.js";
import { AudioEventV1 } from "../src/audio-event.js";
import { ChatMessageV1 } from "../src/chat-message.js";
import { CombatRealtimeEventV1, CombatSummaryV1 } from "../src/combat-event.js";
import { DeathInsightRequestV1 } from "../src/death-insight.js";
import {
  HeartDemonOfferDraftV1,
  HeartDemonPregenRequestV1,
} from "../src/heart-demon.js";
import { validateBiographyEntryV1Contract } from "../src/biography.js";
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
import { CHANNELS, REDIS_V1_CHANNELS } from "../src/channels.js";
import * as SchemaPackage from "../src/index.js";
import { NarrationV1, validateNarrationV1Contract } from "../src/narration.js";
import {
  FactionEventV1,
  NpcDeathV1,
  NpcSpawnedV1,
} from "../src/npc.js";
import { RealmVisionParamsV1 } from "../src/realm-vision.js";
import { ClientRequestV1 } from "../src/client-request.js";
import { ServerDataV1 } from "../src/server-data.js";
import {
  TsyNpcSpawnedV1,
  TsySentinelPhaseChangedV1,
} from "../src/tsy-hostile-v1.js";
import {
  AlchemyInterventionResultV1,
  AlchemySessionEndV1,
  AlchemySessionStartV1,
} from "../src/alchemy.js";
import {
  ForgeOutcomePayloadV1,
  ForgeStartPayloadV1,
} from "../src/forge-bridge.js";
import {
  SkillCapChangedPayloadV1,
  SkillLvUpPayloadV1,
  SkillSnapshotPayloadV1,
  SkillScrollUsedPayloadV1,
  SkillXpGainPayloadV1,
} from "../src/skill.js";
import {
  SocialExposureEventV1,
  SocialFeudEventV1,
  SocialPactEventV1,
  SocialRenownDeltaV1,
} from "../src/social.js";
import { SpiritualSenseTargetsV1 } from "../src/spiritual-sense.js";
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
  it("declares social Redis channels", () => {
    expect(CHANNELS.SOCIAL_EXPOSURE).toBe("bong:social/exposure");
    expect(CHANNELS.SOCIAL_PACT).toBe("bong:social/pact");
    expect(CHANNELS.SOCIAL_FEUD).toBe("bong:social/feud");
    expect(CHANNELS.SOCIAL_RENOWN_DELTA).toBe("bong:social/renown_delta");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SOCIAL_EXPOSURE);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SOCIAL_RENOWN_DELTA);
  });

  it("declares alchemy Redis channels", () => {
    expect(CHANNELS.ALCHEMY_SESSION_START).toBe("bong:alchemy/session_start");
    expect(CHANNELS.ALCHEMY_SESSION_END).toBe("bong:alchemy/session_end");
    expect(CHANNELS.ALCHEMY_INTERVENTION_RESULT).toBe(
      "bong:alchemy/intervention_result",
    );
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ALCHEMY_SESSION_START);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ALCHEMY_SESSION_END);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ALCHEMY_INTERVENTION_RESULT);
  });

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

  it("inventory-snapshot.sample.json carries forge item metadata", () => {
    const data = loadSample("inventory-snapshot.sample.json");
    const result = validate(InventorySnapshotV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
    expect(data.equipped.main_hand.forge_quality).toBe(0.74);
    expect(data.equipped.main_hand.forge_color).toBe("Sharp");
    expect(data.equipped.main_hand.forge_side_effects).toEqual(["brittle_edge"]);
    expect(data.equipped.main_hand.forge_achieved_tier).toBe(1);
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

  it("validates NPC and faction event payloads", () => {
    expect(validate(NpcSpawnedV1, {
      v: 1,
      kind: "npc_spawned",
      npc_id: "npc_1v1",
      archetype: "rogue",
      source: "agent_command",
      zone: "spawn",
      pos: [1, 66, 2],
      initial_age_ticks: 0,
      at_tick: 0,
    }).ok).toBe(true);

    expect(validate(NpcDeathV1, {
      v: 1,
      kind: "npc_death",
      npc_id: "npc_1v1",
      archetype: "commoner",
      cause: "natural_aging",
      age_ticks: 10,
      max_age_ticks: 10,
      at_tick: 0,
    }).ok).toBe(true);

    expect(validate(FactionEventV1, {
      v: 1,
      kind: "faction_event",
      faction_id: "attack",
      event_kind: "adjust_loyalty_bias",
      loyalty_bias: 0.6,
      mission_queue_size: 1,
      at_tick: 0,
    }).ok).toBe(true);
  });

  it("server-data.botany-harvest-progress.sample.json", () => {
    const data = loadSample("server-data.botany-harvest-progress.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.botany-plant-v2-render-profiles.sample.json", () => {
    const data = loadSample("server-data.botany-plant-v2-render-profiles.sample.json");
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

  it("server-data.heart-demon-offer.sample.json", () => {
    const data = loadSample("server-data.heart-demon-offer.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("accepts heart demon pregen request and offer draft", () => {
    const request = {
      trigger_id: "heart_demon:1:1000",
      character_id: "offline:Azure",
      actor_name: "Azure",
      realm: "Spirit",
      qi_color_state: { main: "Mellow", is_chaotic: false, is_hunyuan: false },
      recent_biography: ["t240:reach:Spirit"],
      composure: 0.7,
      started_tick: 1000,
      waves_total: 5,
    };
    const draft = {
      offer_id: "heart_demon:1:1000",
      trigger_id: "heart_demon:1:1000",
      trigger_label: "心魔劫临身",
      realm_label: "渡虚劫 · 心魔",
      composure: 0.7,
      quota_remaining: 1,
      quota_total: 1,
      expires_at_ms: 123,
      choices: [
        {
          choice_id: "heart_demon_choice_0",
          category: "Composure",
          title: "守本心",
          effect_summary: "稳住心神，回复少量当前真元",
          flavor: "旧事浮起，仍可守心。",
          style_hint: "稳妥",
        },
      ],
    };

    expect(validate(HeartDemonPregenRequestV1, request).ok).toBe(true);
    expect(validate(HeartDemonOfferDraftV1, draft).ok).toBe(true);
  });

  it("server-data.tribulation-broadcast.sample.json", () => {
    const data = loadSample("server-data.tribulation-broadcast.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.burst-meridian-event.sample.json", () => {
    const data = loadSample("server-data.burst-meridian-event.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  for (const sample of [
    "server-data.forge-station.sample.json",
    "server-data.forge-session.sample.json",
    "server-data.forge-outcome-perfect.sample.json",
    "server-data.forge-outcome-flawed.sample.json",
    "server-data.forge-blueprint-book.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(ServerDataV1, data);
      expect(result.ok, result.errors.join("; ")).toBe(true);
    });
  }

  for (const sample of [
    "server-data.social-anonymity.sample.json",
    "server-data.social-exposure.sample.json",
    "server-data.social-pact.sample.json",
    "server-data.social-feud.sample.json",
    "server-data.social-renown-delta.sample.json",
    "server-data.sparring-invite.sample.json",
    "server-data.trade-offer.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(ServerDataV1, data);
      expect(result.ok, result.errors.join("; ")).toBe(true);
    });
  }

  for (const sample of [
    "realm-vision-awaken.sample.json",
    "realm-vision-induce.sample.json",
    "realm-vision-condense.sample.json",
    "realm-vision-solidify.sample.json",
    "realm-vision-spirit.sample.json",
    "realm-vision-void.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(RealmVisionParamsV1, data);
      expect(result.ok, result.errors.join("; ")).toBe(true);
    });
  }

  for (const sample of [
    "spiritual-sense-induce.sample.json",
    "spiritual-sense-condense.sample.json",
    "spiritual-sense-solidify.sample.json",
    "spiritual-sense-spirit.sample.json",
    "spiritual-sense-void.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(SpiritualSenseTargetsV1, data);
      expect(result.ok, result.errors.join("; ")).toBe(true);
    });
  }

  for (const sample of [
    "server-data.realm-vision-params.sample.json",
    "server-data.spiritual-sense-targets.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(ServerDataV1, data);
      expect(result.ok, result.errors.join("; ")).toBe(true);
    });
  }

  it("server-data.skillbar-config.sample.json", () => {
    const data = loadSample("server-data.skillbar-config.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.techniques-snapshot.sample.json", () => {
    const data = loadSample("server-data.techniques-snapshot.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.weapon-equipped.sample.json", () => {
    const data = loadSample("server-data.weapon-equipped.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.weapon-equipped-empty.sample.json", () => {
    const data = loadSample("server-data.weapon-equipped-empty.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.weapon-broken.sample.json", () => {
    const data = loadSample("server-data.weapon-broken.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.treasure-equipped.sample.json", () => {
    const data = loadSample("server-data.treasure-equipped.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  for (const sample of [
    "server-data.container-state.sample.json",
    "server-data.search-started.sample.json",
    "server-data.search-progress.sample.json",
    "server-data.search-completed.sample.json",
    "server-data.search-aborted.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(ServerDataV1, data);
      expect(result.ok, result.errors.join("; ")).toBe(true);
    });
  }

  for (const sample of [
    "client-request.alchemy-open-furnace.sample.json",
    "client-request.alchemy-feed-slot.sample.json",
    "client-request.alchemy-take-back.sample.json",
    "client-request.alchemy-ignite.sample.json",
    "client-request.alchemy-intervention.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(ClientRequestV1, data);
      expect(result.ok, result.errors.join("; ")).toBe(true);
    });
  }

  it("rejects stale alchemy furnace_id routing", () => {
    const result = validate(ClientRequestV1, {
      v: 1,
      type: "alchemy_open_furnace",
      furnace_id: "block_-12_64_38",
    });
    expect(result.ok).toBe(false);
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

  for (const sample of [
    "client-request.start-search.sample.json",
    "client-request.cancel-search.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(ClientRequestV1, data);
      expect(result.ok, result.errors.join("; ")).toBe(true);
    });
  }

  for (const sample of [
    "client-request.forge-start.sample.json",
    "client-request.forge-tempering-hit.sample.json",
    "client-request.forge-inscription-submit.sample.json",
    "client-request.forge-consecration-inject.sample.json",
    "client-request.forge-station-place.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(ClientRequestV1, data);
      expect(result.ok, result.errors.join("; ")).toBe(true);
    });
  }

  it("client-request.use-quick-slot.sample.json", () => {
    const data = loadSample("client-request.use-quick-slot.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.quick-slot-bind.sample.json", () => {
    const data = loadSample("client-request.quick-slot-bind.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.skill-bar-bind.sample.json", () => {
    const data = loadSample("client-request.skill-bar-bind.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.skill-bar-cast.sample.json", () => {
    const data = loadSample("client-request.skill-bar-cast.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.trade-offer-response.sample.json", () => {
    const data = loadSample("client-request.trade-offer-response.sample.json");
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

  it("vfx-event.play-anim-inline.sample.json", () => {
    const data = loadSample("vfx-event.play-anim-inline.sample.json");
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

  it("audio-event.play-sound-recipe.sample.json", () => {
    const data = loadSample("audio-event.play-sound-recipe.sample.json");
    const result = validate(AudioEventV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("audio-event.stop-sound-recipe.sample.json", () => {
    const data = loadSample("audio-event.stop-sound-recipe.sample.json");
    const result = validate(AudioEventV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("social redis event samples", () => {
    expect(
      validate(SocialExposureEventV1, loadSample("social-exposure-event.sample.json")).ok,
    ).toBe(true);
    expect(
      validate(SocialPactEventV1, loadSample("social-pact-event.sample.json")).ok,
    ).toBe(true);
    expect(
      validate(SocialFeudEventV1, loadSample("social-feud-event.sample.json")).ok,
    ).toBe(true);
    expect(
      validate(SocialRenownDeltaV1, loadSample("social-renown-delta.sample.json")).ok,
    ).toBe(true);
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

  it("rejects invalid burst meridian event", () => {
    const data = loadObjectSample("server-data.burst-meridian-event.sample.json");
    data.overload_ratio = -1;
    const result = validate(ServerDataV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects nested social event version inside server_data payload", () => {
    const data = loadObjectSample("server-data.social-exposure.sample.json");
    data.event_v = 1;
    const result = validate(ServerDataV1, data);
    expect(result.ok).toBe(false);
  });

  it("client-request.forge-station-place.invalid-missing-tier.sample.json", () => {
    const data = loadSample("client-request.forge-station-place.invalid-missing-tier.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok).toBe(false);
  });

  it("client-request.trade-offer-response.invalid-null-instance.sample.json", () => {
    const data = loadSample("client-request.trade-offer-response.invalid-null-instance.sample.json");
    const result = validate(ClientRequestV1, data);
    expect(result.ok).toBe(false);
  });

  for (const sample of [
    "client-request.start-search.invalid-missing-type.sample.json",
    "client-request.start-search.invalid-negative-id.sample.json",
  ]) {
    it(sample, () => {
      const data = loadSample(sample);
      const result = validate(ClientRequestV1, data);
      expect(result.ok).toBe(false);
    });
  }

  it("rejects extra weapon payload fields", () => {
    const data = loadObjectSample("server-data.weapon-equipped.sample.json");
    data.unexpected = true;
    const result = validate(ServerDataV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects tribulation broadcast payload drift", () => {
    const data = loadObjectSample("server-data.tribulation-broadcast.sample.json");
    delete data.spectate_distance;
    data.actor_id = "offline:YanWujiu";
    const result = validate(ServerDataV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects invalid skill bar binding union", () => {
    const data = {
      v: 1,
      type: "skill_bar_bind",
      slot: 0,
      binding: { kind: "skill", template_id: "wrong_field" },
    };
    const result = validate(ClientRequestV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects out-of-range quick slot request", () => {
    const data = { v: 1, type: "use_quick_slot", slot: 9 };
    const result = validate(ClientRequestV1, data);
    expect(result.ok).toBe(false);
  });

  it("rejects malformed skillbar_config slot entry", () => {
    const data = loadObjectSample("server-data.skillbar-config.sample.json");
    const slots = data.slots as Array<Record<string, unknown> | null>;
    slots[0] = { kind: "skill", display_name: "崩拳" };
    const result = validate(ServerDataV1, data);
    expect(result.ok).toBe(false);
  });
});

describe("forge bridge payload samples pass schema validation", () => {
  it("forge-start-payload.sample.json", () => {
    const data = loadSample("forge-start-payload.sample.json");
    const result = validate(ForgeStartPayloadV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("forge-outcome-payload-perfect.sample.json", () => {
    const data = loadSample("forge-outcome-payload-perfect.sample.json");
    const result = validate(ForgeOutcomePayloadV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("forge-outcome-payload-flawed.sample.json", () => {
    const data = loadSample("forge-outcome-payload-flawed.sample.json");
    const result = validate(ForgeOutcomePayloadV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });
});

describe("alchemy bridge payload samples pass schema validation", () => {
  it("alchemy-session-start.sample.json", () => {
    const data = loadSample("alchemy-session-start.sample.json");
    const result = validate(AlchemySessionStartV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("alchemy-session-end-explode.sample.json", () => {
    const data = loadSample("alchemy-session-end-explode.sample.json");
    const result = validate(AlchemySessionEndV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("alchemy-intervention-result.sample.json", () => {
    const data = loadSample("alchemy-intervention-result.sample.json");
    const result = validate(AlchemyInterventionResultV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
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

  it("accepts tribulation interception biography tags", () => {
    expectContractAccepts(
      "BiographyEntryV1 TribulationIntercepted tag",
      validateBiographyEntryV1Contract,
      {
        TribulationIntercepted: {
          victim_id: "offline:Victim",
          tag: "戮道者 · 截劫",
          tick: 120,
        },
      },
    );
    expectContractAccepts(
      "BiographyEntryV1 legacy TribulationIntercepted without tag",
      validateBiographyEntryV1Contract,
      {
        TribulationIntercepted: {
          victim_id: "offline:Victim",
          tick: 120,
        },
      },
    );
  });

  it("accepts social snapshots in world state", () => {
    const data = loadObjectSample("world-state.sample.json");
    const firstPlayer = (data.players as Array<Record<string, unknown>>)[0];
    const social = firstPlayer.social as Record<string, unknown>;
    expect((social.renown as Record<string, unknown>).fame).toBe(2);
    expect(Array.isArray(social.relationships)).toBe(true);

    expectContractAccepts(
      "WorldStateV1 player social snapshot",
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
    data.commands = [{ type: "spawn_npc", target: "spawn", params: { archetype: "beast", count: 2 } }];
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

  it("rejects unsupported npc behavior and faction params", () => {
    const badBehavior = loadObjectSample("agent-command.sample.json");
    badBehavior.commands = [{ type: "npc_behavior", target: "npc_1v1", params: {} }];
    expectContractRejects(
      "AgentCommandV1 npc_behavior requires flee_threshold",
      validateAgentCommandV1Contract,
      badBehavior,
    );

    const badFaction = loadObjectSample("agent-command.sample.json");
    badFaction.commands = [
      { type: "faction_event", target: "neutral", params: { kind: "invent", faction_id: "sky" } },
    ];
    expectContractRejects(
      "AgentCommandV1 faction_event kind/faction whitelist",
      validateAgentCommandV1Contract,
      badFaction,
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
