import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  AgentCommandV1,
  validateAgentCommandV1Contract,
} from "../src/agent-command.js";
import { AntiCheatReportV1 } from "../src/anticheat.js";
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
import {
  AlchemyItemDataV1,
  InventoryEventV1,
  InventorySnapshotV1,
} from "../src/inventory.js";
import {
  ZonePressureCrossedV1,
  validateZonePressureCrossedV1Contract,
} from "../src/zone-pressure.js";
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
import {
  PseudoVeinDissipateEventV1,
  PseudoVeinSnapshotV1,
} from "../src/pseudo-vein.js";
import {
  WeatherEventDataV1,
  WeatherEventKindV1,
  WeatherEventUpdateV1,
} from "../src/lingtian-weather.js";
import {
  RatPhaseChangeEventV1,
  validateRatPhaseChangeEventV1Contract,
} from "../src/rat-phase-event.js";
import { ZongCoreActivationV1 } from "../src/zong-formation.js";
import { RealmVisionParamsV1 } from "../src/realm-vision.js";
import { ClientRequestV1 } from "../src/client-request.js";
import { ServerDataV1 } from "../src/server-data.js";
import {
  TsyNpcSpawnedV1,
  TsySentinelPhaseChangedV1,
} from "../src/tsy-hostile-v1.js";
import {
  AlchemyInsightV1,
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
  NicheGuardianBrokenV1,
  NicheGuardianFatigueV1,
  NicheIntrusionEventV1,
  HighRenownMilestoneEventV1,
  SocialPactEventV1,
  SocialRenownDeltaV1,
} from "../src/social.js";
import {
  SpiritEyeDiscoveredV1,
  SpiritEyeMigrateV1,
  SpiritEyeUsedForBreakthroughV1,
} from "../src/spirit-eye.js";
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
    expect(CHANNELS.SOCIAL_NICHE_INTRUSION).toBe("bong:social/niche_intrusion");
    expect(CHANNELS.HIGH_RENOWN_MILESTONE).toBe("bong:high_renown_milestone");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SOCIAL_EXPOSURE);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SOCIAL_RENOWN_DELTA);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SOCIAL_NICHE_INTRUSION);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.HIGH_RENOWN_MILESTONE);
  });

  it("declares alchemy Redis channels", () => {
    expect(CHANNELS.ALCHEMY_SESSION_START).toBe("bong:alchemy/session_start");
    expect(CHANNELS.ALCHEMY_SESSION_END).toBe("bong:alchemy/session_end");
    expect(CHANNELS.ALCHEMY_INTERVENTION_RESULT).toBe(
      "bong:alchemy/intervention_result",
    );
    expect(CHANNELS.ALCHEMY_INSIGHT).toBe("bong:alchemy_insight");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ALCHEMY_SESSION_START);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ALCHEMY_SESSION_END);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ALCHEMY_INTERVENTION_RESULT);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ALCHEMY_INSIGHT);
  });

  it("declares anticheat Redis channel", () => {
    expect(CHANNELS.ANTICHEAT).toBe("bong:anticheat");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ANTICHEAT);
  });

  it("declares season changed Redis channel", () => {
    expect(CHANNELS.SEASON_CHANGED).toBe("bong:season_changed");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SEASON_CHANGED);
  });

  it("declares pseudo vein Redis channels", () => {
    expect(CHANNELS.PSEUDO_VEIN_ACTIVE).toBe("bong:pseudo_vein:active");
    expect(CHANNELS.PSEUDO_VEIN_DISSIPATE).toBe("bong:pseudo_vein:dissipate");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.PSEUDO_VEIN_ACTIVE);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.PSEUDO_VEIN_DISSIPATE);
  });

  it("declares weather event update Redis channel", () => {
    expect(CHANNELS.WEATHER_EVENT_UPDATE).toBe("bong:weather_event_update");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.WEATHER_EVENT_UPDATE);
  });

  it("declares zong formation Redis channel", () => {
    expect(CHANNELS.ZONG_CORE_ACTIVATED).toBe("bong:zong_core_activated");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ZONG_CORE_ACTIVATED);
  });

  it("declares spirit eye Redis channels", () => {
    expect(CHANNELS.SPIRIT_EYE_MIGRATE).toBe("bong:spirit_eye/migrate");
    expect(CHANNELS.SPIRIT_EYE_DISCOVERED).toBe("bong:spirit_eye/discovered");
    expect(CHANNELS.SPIRIT_EYE_USED_FOR_BREAKTHROUGH).toBe(
      "bong:spirit_eye/used_for_breakthrough",
    );
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SPIRIT_EYE_MIGRATE);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SPIRIT_EYE_DISCOVERED);
    expect(REDIS_V1_CHANNELS).toContain(
      CHANNELS.SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
    );
  });

  it("declares zone pressure Redis channel", () => {
    expect(CHANNELS.ZONE_PRESSURE_CROSSED).toBe("bong:zone/pressure_crossed");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.ZONE_PRESSURE_CROSSED);
  });

  it("declares rat phase Redis channel", () => {
    expect(CHANNELS.RAT_PHASE_EVENT).toBe("bong:rat_phase_event");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.RAT_PHASE_EVENT);
  });

  it("world-state.sample.json", () => {
    const data = loadSample("world-state.sample.json");
    const result = validate(WorldStateV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("rat phase event contract", () => {
    const data = {
      chunk: [8, 8],
      zone: "spawn",
      group_id: 7,
      from: "solitary",
      to: { transitioning: { progress: 0 } },
      rat_count: 12,
      local_qi: 0.42,
      qi_gradient: 0.31,
      tick: 12345,
    };
    expect(validate(RatPhaseChangeEventV1, data).ok).toBe(true);
    expectContractAccepts(
      "RatPhaseChangeEventV1",
      validateRatPhaseChangeEventV1Contract,
      data,
    );
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

  it("pseudo-vein-snapshot.sample.json", () => {
    const data = loadSample("pseudo-vein-snapshot.sample.json");
    const result = validate(PseudoVeinSnapshotV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("pseudo-vein-dissipate-event.sample.json", () => {
    const data = loadSample("pseudo-vein-dissipate-event.sample.json");
    const result = validate(PseudoVeinDissipateEventV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("weather-event-data.sample.json", () => {
    const data = loadSample("weather-event-data.sample.json");
    const result = validate(WeatherEventDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("weather-event-update.sample.json", () => {
    const data = loadSample("weather-event-update.sample.json");
    const result = validate(WeatherEventUpdateV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("zong-core-activation.sample.json", () => {
    const data = loadSample("zong-core-activation.sample.json");
    const result = validate(ZongCoreActivationV1, data);
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

  it("alchemy item data accepts pill residue metadata", () => {
    const result = validate(AlchemyItemDataV1, {
      kind: "pill_residue",
      residue_kind: "failed_pill",
      produced_at_tick: 120,
      expires_at_tick: 5_184_120,
    });
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

  it("server-data.lumber-progress.sample.json", () => {
    const data = loadSample("server-data.lumber-progress.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("server-data.botany-skill.sample.json", () => {
    const data = loadSample("server-data.botany-skill.sample.json");
    const result = validate(ServerDataV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("zone pressure contract accepts rising pressure events", () => {
    const data = {
      v: 1,
      kind: "zone_pressure_crossed",
      zone: "starter_zone",
      level: "high",
      raw_pressure: 1.25,
      at_tick: 1440,
    };

    expect(validate(ZonePressureCrossedV1, data).ok).toBe(true);
    expectContractAccepts("ZonePressureCrossedV1", validateZonePressureCrossedV1Contract, data);
    expectContractRejects("ZonePressureCrossedV1", validateZonePressureCrossedV1Contract, {
      ...data,
      level: "none",
    });
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
    "server-data.niche-intrusion.sample.json",
    "server-data.niche-guardian-fatigue.sample.json",
    "server-data.niche-guardian-broken.sample.json",
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

  it("server-data.skill-config-snapshot.sample.json", () => {
    const data = loadSample("server-data.skill-config-snapshot.sample.json");
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

  it("client-request.lingtian_start_replenish accepts pill residue source", () => {
    const result = validate(ClientRequestV1, {
      v: 1,
      type: "lingtian_start_replenish",
      x: 1,
      y: 64,
      z: -2,
      source: "pill_residue_failed_pill",
    });
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-request.lingtian_start_replenish rejects unknown replenish source", () => {
    const result = validate(ClientRequestV1, {
      v: 1,
      type: "lingtian_start_replenish",
      x: 1,
      y: 64,
      z: -2,
      source: "raw_sludge",
    });
    expect(result.ok).toBe(false);
  });

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

  it("client-request.skill-config-intent.sample.json", () => {
    const data = loadSample("client-request.skill-config-intent.sample.json");
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

  it("anticheat-report.sample.json", () => {
    const data = loadSample("anticheat-report.sample.json");
    const result = validate(AntiCheatReportV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("death-insight-request.sample.json", () => {
    const data = loadSample("death-insight-request.sample.json");
    const result = validate(DeathInsightRequestV1, data);
    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("accepts spirit eye event contracts", () => {
    const migrate = validate(SpiritEyeMigrateV1, {
      v: 1,
      eye_id: "spirit_eye:spawn:0",
      from: { x: 0, y: 66, z: 0 },
      to: { x: 640, y: 66, z: 0 },
      reason: "usage_pressure",
      usage_pressure: 0,
      tick: 120,
    });
    expect(migrate.ok, migrate.errors.join("; ")).toBe(true);

    const discovered = validate(SpiritEyeDiscoveredV1, {
      v: 1,
      eye_id: "spirit_eye:spawn:0",
      character_id: "char:alice",
      pos: { x: 14, y: 66, z: 14 },
      zone: "spawn",
      qi_concentration: 1.0,
      discovered_at_tick: 77,
    });
    expect(discovered.ok, discovered.errors.join("; ")).toBe(true);

    const used = validate(SpiritEyeUsedForBreakthroughV1, {
      v: 1,
      eye_id: "spirit_eye:spawn:0",
      character_id: "char:alice",
      realm_from: "Condense",
      realm_to: "Solidify",
      usage_pressure: 0.1,
      tick: 88,
    });
    expect(used.ok, used.errors.join("; ")).toBe(true);
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
    expect(
      validate(NicheIntrusionEventV1, loadSample("niche-intrusion-event.sample.json")).ok,
    ).toBe(true);
    expect(
      validate(NicheGuardianFatigueV1, loadSample("niche-guardian-fatigue.sample.json")).ok,
    ).toBe(true);
    expect(
      validate(NicheGuardianBrokenV1, loadSample("niche-guardian-broken.sample.json")).ok,
    ).toBe(true);
    expect(
      validate(
        HighRenownMilestoneEventV1,
        loadSample("high-renown-milestone-event.sample.json"),
      ).ok,
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

  it("rejects non-object skill config intent payload", () => {
    const data = loadObjectSample("client-request.skill-config-intent.sample.json");
    data.config = null;
    const result = validate(ClientRequestV1, data);
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

  it("alchemy-insight.sample.json", () => {
    const data = loadSample("alchemy-insight.sample.json");
    const result = validate(AlchemyInsightV1, data);
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

describe("plan-lingtian-weather-v1 §4.2 schema", () => {
  it("WeatherEventKindV1 接受 5 个 wire 字符串", () => {
    for (const kind of [
      "thunderstorm",
      "drought_wind",
      "blizzard",
      "heavy_haze",
      "ling_mist",
    ]) {
      const result = validate(WeatherEventKindV1, kind);
      expect(result.ok, `kind=${kind}: ${result.errors.join("; ")}`).toBe(true);
    }
  });

  it("WeatherEventKindV1 拒绝未知 wire 字符串", () => {
    const result = validate(WeatherEventKindV1, "tornado");
    expect(result.ok).toBe(false);
  });

  it("WeatherEventDataV1 拒绝缺字段", () => {
    const data = {
      v: 1,
      zone_id: "default",
      kind: "thunderstorm",
      // 缺 started_at_lingtian_tick
      expires_at_lingtian_tick: 200,
      remaining_ticks: 100,
    };
    const result = validate(WeatherEventDataV1, data);
    expect(result.ok).toBe(false);
  });

  it("WeatherEventDataV1 拒绝负 tick", () => {
    const data = {
      v: 1,
      zone_id: "default",
      kind: "thunderstorm",
      started_at_lingtian_tick: -1,
      expires_at_lingtian_tick: 200,
      remaining_ticks: 100,
    };
    const result = validate(WeatherEventDataV1, data);
    expect(result.ok).toBe(false);
  });

  it("WeatherEventUpdateV1 接受 started/expired 两种 kind", () => {
    for (const kind of ["started", "expired"]) {
      const data = {
        v: 1,
        kind,
        data: {
          v: 1,
          zone_id: "default",
          kind: "thunderstorm",
          started_at_lingtian_tick: 0,
          expires_at_lingtian_tick: 200,
          remaining_ticks: 100,
        },
      };
      const result = validate(WeatherEventUpdateV1, data);
      expect(result.ok, `kind=${kind}: ${result.errors.join("; ")}`).toBe(true);
    }
  });

  it("WeatherEventUpdateV1 拒绝未知 kind（含原 cleared 历史变体）", () => {
    for (const kind of ["unknown", "cleared"]) {
      const data = {
        v: 1,
        kind,
        data: {
          v: 1,
          zone_id: "default",
          kind: "thunderstorm",
          started_at_lingtian_tick: 0,
          expires_at_lingtian_tick: 200,
          remaining_ticks: 100,
        },
      };
      const result = validate(WeatherEventUpdateV1, data);
      expect(result.ok).toBe(false);
    }
  });
});
