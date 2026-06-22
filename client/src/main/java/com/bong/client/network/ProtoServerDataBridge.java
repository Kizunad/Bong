package com.bong.client.network;

import bong.Envelope;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonNull;
import com.google.gson.JsonPrimitive;
import com.google.protobuf.MessageOrBuilder;
import com.google.protobuf.util.JsonFormat;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.EnumMap;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;
import java.util.regex.Pattern;

/**
 * Proto→legacy-JSON bridge for the {@code bong:server_data} channel.
 * <p>
 * The server now sends protobuf-encoded {@link Envelope.ServerDataEnvelope} on
 * {@code bong:server_data}. Existing 49+ handler classes still consume
 * {@link ServerDataEnvelope} (the legacy JSON wrapper), so this bridge:
 * <ol>
 *   <li>Parses raw bytes as {@code ServerDataEnvelope} proto message</li>
 *   <li>Extracts the inner oneof message via {@code getPayloadCase()}</li>
 *   <li>Converts the inner message to JSON with {@link JsonFormat}</li>
 *   <li>Wraps the result into the legacy {@code {"v":1,"type":"...","...":"..."}} envelope</li>
 * </ol>
 * <p>
 * This avoids rewriting all handlers while the wire format has been flipped to proto.
 */
public final class ProtoServerDataBridge {

    private static final Logger LOG = LoggerFactory.getLogger(ProtoServerDataBridge.class);

    private static final JsonFormat.Printer PRINTER = JsonFormat.printer()
            .preservingProtoFieldNames()
            .includingDefaultValueFields()
            .omittingInsignificantWhitespace();

    /** PayloadCase → handler type string used by {@link ServerDataRouter}. */
    private static final Map<Envelope.ServerDataEnvelope.PayloadCase, String> CASE_TO_TYPE;

    static {
        Map<Envelope.ServerDataEnvelope.PayloadCase, String> m =
                new EnumMap<>(Envelope.ServerDataEnvelope.PayloadCase.class);
        m.put(Envelope.ServerDataEnvelope.PayloadCase.WELCOME, "welcome");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.HEARTBEAT, "heartbeat");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.NARRATION, "narration");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ZONE_INFO, "zone_info");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.PLAYER_STATE, "player_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.CULTIVATION_DETAIL, "cultivation_detail");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SKILL_XP_GAIN, "skill_xp_gain");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.INVENTORY_SNAPSHOT, "inventory_snapshot");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.COMBAT_HUD_STATE, "combat_hud_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.KNOCKBACK_SYNC, "knockback_sync");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ALCHEMY_FURNACE, "alchemy_furnace");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ALCHEMY_SESSION, "alchemy_session");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ALCHEMY_OUTCOME_FORECAST, "alchemy_outcome_forecast");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ALCHEMY_OUTCOME_RESOLVED, "alchemy_outcome_resolved");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ALCHEMY_RECIPE_BOOK, "alchemy_recipe_book");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ALCHEMY_CONTAMINATION, "alchemy_contamination");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FORGE_STATION, "forge_station");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FORGE_SESSION, "forge_session");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FORGE_OUTCOME, "forge_outcome");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FORGE_BLUEPRINT_BOOK, "forge_blueprint_book");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.CRAFT_RECIPE_LIST, "craft_recipe_list");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.CRAFT_SESSION_STATE, "craft_session_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.CRAFT_OUTCOME, "craft_outcome");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.RECIPE_UNLOCKED, "recipe_unlocked");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.BOTANY_HARVEST_PROGRESS, "botany_harvest_progress");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.BOTANY_PLANT_V2_RENDER_PROFILES, "botany_plant_v2_render_profiles");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.BOTANY_SKILL, "botany_skill");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.MINING_PROGRESS, "mining_progress");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.LUMBER_PROGRESS, "lumber_progress");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.GATHERING_SESSION, "gathering_session");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.LINGTIAN_SESSION, "lingtian_session");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.WOUNDS_SNAPSHOT, "wounds_snapshot");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.DEFENSE_WINDOW, "defense_window");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.CAST_SYNC, "cast_sync");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.QUICK_SLOT_CONFIG, "quickslot_config");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SKILL_BAR_CONFIG, "skillbar_config");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.TECHNIQUES_SNAPSHOT, "techniques_snapshot");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.UNLOCKS_SYNC, "unlocks_sync");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.DERIVED_ATTRS_SYNC, "derived_attrs_sync");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.EVENT_STREAM_PUSH, "event_stream_push");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.WEAPON_EQUIPPED, "weapon_equipped");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.WEAPON_BROKEN, "weapon_broken");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SHIELD_BROKEN, "shield_broken");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SHIELD_BLOCK_HIT, "shield_block_hit");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.TREASURE_EQUIPPED, "treasure_equipped");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.VORTEX_STATE, "vortex_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.DUGU_POISON_STATE, "dugu_poison_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.POISON_DOSE_EVENT, "poison_dose_event");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.POISON_OVERDOSE_EVENT, "poison_overdose_event");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.POISON_TRAIT_STATE, "poison_trait_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.CARRIER_STATE, "carrier_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FALSE_SKIN_STATE, "false_skin_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.COMBAT_EVENT_FLOATER, "combat_event");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.TECHNIQUE_PROFICIENCY_UPDATE, "technique_proficiency_update");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.PILL_BUFF_STATUS, "pill_buff_status");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SKILL_CONFIG_SNAPSHOT, "skill_config_snapshot");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SOCIAL_ANONYMITY, "social_anonymity");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SOCIAL_EXPOSURE, "social_exposure");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SOCIAL_PACT, "social_pact");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SOCIAL_FEUD, "social_feud");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SOCIAL_RENOWN_DELTA, "social_renown_delta");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.IDENTITY_PANEL_STATE, "identity_panel_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.NICHE_INTRUSION, "niche_intrusion");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.NICHE_GUARDIAN_FATIGUE, "niche_guardian_fatigue");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.NICHE_GUARDIAN_BROKEN, "niche_guardian_broken");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SPARRING_INVITE, "sparring_invite");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.TRADE_OFFER, "trade_offer");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.TRIBULATION_STATE, "tribulation_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.TRIBULATION_BROADCAST, "tribulation_broadcast");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ASCENSION_QUOTA, "ascension_quota");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.HEART_DEMON_OFFER, "heart_demon_offer");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.BURST_MERIDIAN_EVENT, "burst_meridian_event");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.BREAKTHROUGH_CINEMATIC, "breakthrough_cinematic");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.DEATH_SCREEN, "death_screen");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.TERMINATE_SCREEN, "terminate_screen");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.QI_COLOR_OBSERVED, "qi_color_observed");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.REALM_VISION_PARAMS, "realm_vision_params");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SPIRITUAL_SENSE_TARGETS, "spiritual_sense_targets");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.EVENT_ALERT, "event_alert");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.COFFIN_STATE, "coffin_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.UI_OPEN, "ui_open");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.INVENTORY_EVENT, "inventory_event");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.DROPPED_LOOT_SYNC, "dropped_loot_sync");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.RIFT_PORTAL_STATE, "rift_portal_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.RIFT_PORTAL_REMOVED, "rift_portal_removed");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.EXTRACT_STARTED, "extract_started");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.EXTRACT_PROGRESS, "extract_progress");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.EXTRACT_COMPLETED, "extract_completed");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.EXTRACT_ABORTED, "extract_aborted");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.EXTRACT_FAILED, "extract_failed");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.TSY_COLLAPSE_STARTED, "tsy_collapse_started_ipc");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.CONTAINER_STATE, "container_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SEARCH_STARTED, "search_started");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SEARCH_PROGRESS, "search_progress");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SEARCH_COMPLETED, "search_completed");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SEARCH_ABORTED, "search_aborted");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SKILL_LV_UP, "skill_lv_up");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SKILL_CAP_CHANGED, "skill_cap_changed");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SKILL_SCROLL_USED, "skill_scroll_used");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SKILL_SNAPSHOT, "skill_snapshot");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FULL_POWER_CHARGING, "full_power_charging_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FULL_POWER_RELEASE, "full_power_release");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FULL_POWER_EXHAUSTED, "full_power_exhausted_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.HEALER_NPC_AI_STATE, "healer_npc_ai_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.YIDAO_HUD_STATE, "yidao_hud_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.MOVEMENT_STATE, "movement_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SPIRIT_TREASURE_STATE, "spirit_treasure_state");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SPIRIT_TREASURE_DIALOGUE, "spirit_treasure_dialogue");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.LOOT_CONTAINER_OPEN, "loot_container_open");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.LOOT_CONTAINER_UPDATE, "loot_container_update");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.LOOT_CONTAINER_CLOSE, "loot_container_close");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FACTION_WAR_STATE, "faction_war_state");
        // plan-combat-skill-feedback-bridges-v1 P4：暗器 HUD
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ANQI_HUD, "anqi_hud");
        // plan-combat-skill-feedback-bridges-v1 P5：毒蛊 v2 HUD S2C
        m.put(Envelope.ServerDataEnvelope.PayloadCase.DUGU_V2_SKILL_CAST, "dugu_v2_skill_cast");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.DUGU_V2_SELF_CURE, "dugu_v2_self_cure");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.DUGU_V2_SHROUD_ACTIVE, "dugu_v2_shroud_active");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.PERMANENT_QI_MAX_DECAY_APPLIED, "permanent_qi_max_decay_applied");
        // plan-combat-skill-feedback-bridges-v1 P6：人剑共生 HUD S2C
        m.put(Envelope.ServerDataEnvelope.PayloadCase.SWORD_BOND_HUD_STATE, "sword_bond_hud_state");
        // plan-exploration-probe-return-v1 P0：神识感知矿脉 S2C
        m.put(Envelope.ServerDataEnvelope.PayloadCase.MINERAL_PROBE_RESULT, "mineral_probe_result");
        // plan-exploration-probe-return-v1 P1：神识感知保鲜 S2C（复用 freshness_update 链）
        m.put(Envelope.ServerDataEnvelope.PayloadCase.FRESHNESS_UPDATE, "freshness_update");
        // plan-exploration-probe-return-v1 P2：修炼顿悟 S2C
        m.put(Envelope.ServerDataEnvelope.PayloadCase.INSIGHT_OFFER, "insight_offer");
        m.put(Envelope.ServerDataEnvelope.PayloadCase.WORKBENCH_OPEN, "workbench_open");
        // 功法 AV 里程碑 系列 2b：zhenmai 5 招专属 HUD S2C（server dual-emit → 条件式 HUD）
        m.put(Envelope.ServerDataEnvelope.PayloadCase.ZHENMAI_HUD, "zhenmai_hud");
        CASE_TO_TYPE = m;
    }

    private ProtoServerDataBridge() {}

    /**
     * Result of a bridge attempt. Either a successful conversion to legacy JSON
     * or a failure with an error message.
     */
    public static final class BridgeResult {
        private final String legacyJson;
        private final String errorMessage;

        private BridgeResult(String legacyJson, String errorMessage) {
            this.legacyJson = legacyJson;
            this.errorMessage = errorMessage;
        }

        public boolean isSuccess() {
            return legacyJson != null;
        }

        public String legacyJson() {
            return legacyJson;
        }

        public String errorMessage() {
            return errorMessage;
        }

        static BridgeResult success(String json) {
            return new BridgeResult(json, null);
        }

        static BridgeResult error(String message) {
            return new BridgeResult(null, message);
        }
    }

    /**
     * Try to parse raw bytes as a protobuf {@link Envelope.ServerDataEnvelope},
     * extract the oneof payload, and convert to the legacy JSON envelope format
     * that existing handlers expect.
     *
     * @param rawBytes the raw bytes received on the {@code bong:server_data} channel
     * @return result with legacy JSON string on success, or error info
     */
    public static BridgeResult bridge(byte[] rawBytes) {
        Envelope.ServerDataEnvelope envelope;
        try {
            envelope = Envelope.ServerDataEnvelope.parseFrom(rawBytes);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto parse failed: " + e.getMessage());
        }

        Envelope.ServerDataEnvelope.PayloadCase payloadCase = envelope.getPayloadCase();
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.PAYLOAD_NOT_SET) {
            return BridgeResult.error("proto envelope has no payload set");
        }

        String typeString = CASE_TO_TYPE.get(payloadCase);
        if (typeString == null) {
            return BridgeResult.error("unmapped PayloadCase: " + payloadCase.name());
        }

        // Payloads with oneof/wrapper structures that need flattening to match legacy handlers.
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.INVENTORY_EVENT) {
            return bridgeOneofFlat(envelope.getInventoryEvent(), typeString,
                    ie -> switch (ie.getEventCase()) {
                        case MOVED -> new OneofVariant("moved", ie.getMoved());
                        case DROPPED -> new OneofVariant("dropped", ie.getDropped());
                        case STACK_CHANGED -> new OneofVariant("stack_changed", ie.getStackChanged());
                        case DURABILITY_CHANGED -> new OneofVariant("durability_changed", ie.getDurabilityChanged());
                        default -> null;
                    });
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.CRAFT_OUTCOME) {
            return bridgeOneofFlat(envelope.getCraftOutcome(), typeString,
                    co -> switch (co.getOutcomeCase()) {
                        case COMPLETED -> new OneofVariant("completed", co.getCompleted());
                        case FAILED -> new OneofVariant("failed", co.getFailed());
                        default -> null;
                    });
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.QUICK_SLOT_CONFIG) {
            return bridgeSlotConfig(envelope.getQuickSlotConfig(), typeString, "entry", null);
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.SKILL_BAR_CONFIG) {
            return bridgeSlotConfig(envelope.getSkillBarConfig(), typeString, "entry",
                    ProtoServerDataBridge::flattenSkillBarOneof);
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.FORGE_SESSION) {
            return bridgeForgeSession(envelope.getForgeSession(), typeString);
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.INVENTORY_SNAPSHOT) {
            return bridgeInventorySnapshot(envelope.getInventorySnapshot(), typeString);
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.MOVEMENT_STATE) {
            return bridgeMovementState(envelope.getMovementState(), typeString);
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.CAST_SYNC) {
            return bridgeCastSync(envelope.getCastSync(), typeString);
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.SKILL_CONFIG_SNAPSHOT) {
            return bridgeSkillConfigSnapshot(envelope.getSkillConfigSnapshot(), typeString);
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.SKILL_SNAPSHOT) {
            return bridgeSkillSnapshot(envelope.getSkillSnapshot(), typeString);
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.CULTIVATION_DETAIL) {
            return bridgeCultivationDetail(envelope.getCultivationDetail(), typeString);
        }
        if (payloadCase == Envelope.ServerDataEnvelope.PayloadCase.CRAFT_RECIPE_LIST) {
            return bridgeCraftRecipeList(envelope.getCraftRecipeList(), typeString);
        }

        // Extract the inner oneof message.
        MessageOrBuilder inner = extractInner(envelope, payloadCase);
        if (inner == null) {
            return BridgeResult.error("failed to extract inner message for " + payloadCase.name());
        }

        try {
            String innerJson = printAndNormalize(inner);

            String legacyJson;
            if (innerJson.equals("{}")) {
                legacyJson = "{\"v\":1,\"type\":\"" + typeString + "\"}";
            } else {
                legacyJson = "{\"v\":1,\"type\":\"" + typeString + "\"," + innerJson.substring(1);
            }

            return BridgeResult.success(legacyJson);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + payloadCase.name() + ": " + e.getMessage());
        }
    }

    /**
     * Extract the concrete inner message from the oneof payload.
     */
    @SuppressWarnings("java:S1479") // large switch is intentional — one case per proto variant
    private static MessageOrBuilder extractInner(
            Envelope.ServerDataEnvelope envelope,
            Envelope.ServerDataEnvelope.PayloadCase payloadCase) {
        switch (payloadCase) {
            case WELCOME: return envelope.getWelcome();
            case HEARTBEAT: return envelope.getHeartbeat();
            case NARRATION: return envelope.getNarration();
            case ZONE_INFO: return envelope.getZoneInfo();
            case PLAYER_STATE: return envelope.getPlayerState();
            case CULTIVATION_DETAIL: return envelope.getCultivationDetail();
            case SKILL_XP_GAIN: return envelope.getSkillXpGain();
            case INVENTORY_SNAPSHOT: return envelope.getInventorySnapshot();
            case COMBAT_HUD_STATE: return envelope.getCombatHudState();
            case KNOCKBACK_SYNC: return envelope.getKnockbackSync();
            case ALCHEMY_FURNACE: return envelope.getAlchemyFurnace();
            case ALCHEMY_SESSION: return envelope.getAlchemySession();
            case ALCHEMY_OUTCOME_FORECAST: return envelope.getAlchemyOutcomeForecast();
            case ALCHEMY_OUTCOME_RESOLVED: return envelope.getAlchemyOutcomeResolved();
            case ALCHEMY_RECIPE_BOOK: return envelope.getAlchemyRecipeBook();
            case ALCHEMY_CONTAMINATION: return envelope.getAlchemyContamination();
            case FORGE_STATION: return envelope.getForgeStation();
            case FORGE_SESSION: return envelope.getForgeSession();
            case FORGE_OUTCOME: return envelope.getForgeOutcome();
            case FORGE_BLUEPRINT_BOOK: return envelope.getForgeBlueprintBook();
            case CRAFT_RECIPE_LIST: return envelope.getCraftRecipeList();
            case CRAFT_SESSION_STATE: return envelope.getCraftSessionState();
            case CRAFT_OUTCOME: return envelope.getCraftOutcome();
            case RECIPE_UNLOCKED: return envelope.getRecipeUnlocked();
            case BOTANY_HARVEST_PROGRESS: return envelope.getBotanyHarvestProgress();
            case BOTANY_PLANT_V2_RENDER_PROFILES: return envelope.getBotanyPlantV2RenderProfiles();
            case BOTANY_SKILL: return envelope.getBotanySkill();
            case MINING_PROGRESS: return envelope.getMiningProgress();
            case LUMBER_PROGRESS: return envelope.getLumberProgress();
            case GATHERING_SESSION: return envelope.getGatheringSession();
            case LINGTIAN_SESSION: return envelope.getLingtianSession();
            case WOUNDS_SNAPSHOT: return envelope.getWoundsSnapshot();
            case DEFENSE_WINDOW: return envelope.getDefenseWindow();
            case CAST_SYNC: return envelope.getCastSync();
            case QUICK_SLOT_CONFIG: return envelope.getQuickSlotConfig();
            case SKILL_BAR_CONFIG: return envelope.getSkillBarConfig();
            case TECHNIQUES_SNAPSHOT: return envelope.getTechniquesSnapshot();
            case UNLOCKS_SYNC: return envelope.getUnlocksSync();
            case DERIVED_ATTRS_SYNC: return envelope.getDerivedAttrsSync();
            case EVENT_STREAM_PUSH: return envelope.getEventStreamPush();
            case WEAPON_EQUIPPED: return envelope.getWeaponEquipped();
            case WEAPON_BROKEN: return envelope.getWeaponBroken();
            case SHIELD_BROKEN: return envelope.getShieldBroken();
            case SHIELD_BLOCK_HIT: return envelope.getShieldBlockHit();
            case TREASURE_EQUIPPED: return envelope.getTreasureEquipped();
            case VORTEX_STATE: return envelope.getVortexState();
            case DUGU_POISON_STATE: return envelope.getDuguPoisonState();
            case POISON_DOSE_EVENT: return envelope.getPoisonDoseEvent();
            case POISON_OVERDOSE_EVENT: return envelope.getPoisonOverdoseEvent();
            case POISON_TRAIT_STATE: return envelope.getPoisonTraitState();
            case CARRIER_STATE: return envelope.getCarrierState();
            case FALSE_SKIN_STATE: return envelope.getFalseSkinState();
            case COMBAT_EVENT_FLOATER: return envelope.getCombatEventFloater();
            case TECHNIQUE_PROFICIENCY_UPDATE: return envelope.getTechniqueProficiencyUpdate();
            case PILL_BUFF_STATUS: return envelope.getPillBuffStatus();
            case SKILL_CONFIG_SNAPSHOT: return envelope.getSkillConfigSnapshot();
            case SOCIAL_ANONYMITY: return envelope.getSocialAnonymity();
            case SOCIAL_EXPOSURE: return envelope.getSocialExposure();
            case SOCIAL_PACT: return envelope.getSocialPact();
            case SOCIAL_FEUD: return envelope.getSocialFeud();
            case SOCIAL_RENOWN_DELTA: return envelope.getSocialRenownDelta();
            case IDENTITY_PANEL_STATE: return envelope.getIdentityPanelState();
            case NICHE_INTRUSION: return envelope.getNicheIntrusion();
            case NICHE_GUARDIAN_FATIGUE: return envelope.getNicheGuardianFatigue();
            case NICHE_GUARDIAN_BROKEN: return envelope.getNicheGuardianBroken();
            case SPARRING_INVITE: return envelope.getSparringInvite();
            case TRADE_OFFER: return envelope.getTradeOffer();
            case TRIBULATION_STATE: return envelope.getTribulationState();
            case TRIBULATION_BROADCAST: return envelope.getTribulationBroadcast();
            case ASCENSION_QUOTA: return envelope.getAscensionQuota();
            case HEART_DEMON_OFFER: return envelope.getHeartDemonOffer();
            case BURST_MERIDIAN_EVENT: return envelope.getBurstMeridianEvent();
            case BREAKTHROUGH_CINEMATIC: return envelope.getBreakthroughCinematic();
            case DEATH_SCREEN: return envelope.getDeathScreen();
            case TERMINATE_SCREEN: return envelope.getTerminateScreen();
            case QI_COLOR_OBSERVED: return envelope.getQiColorObserved();
            case REALM_VISION_PARAMS: return envelope.getRealmVisionParams();
            case SPIRITUAL_SENSE_TARGETS: return envelope.getSpiritualSenseTargets();
            case EVENT_ALERT: return envelope.getEventAlert();
            case COFFIN_STATE: return envelope.getCoffinState();
            case UI_OPEN: return envelope.getUiOpen();
            case INVENTORY_EVENT: return envelope.getInventoryEvent();
            case DROPPED_LOOT_SYNC: return envelope.getDroppedLootSync();
            case RIFT_PORTAL_STATE: return envelope.getRiftPortalState();
            case RIFT_PORTAL_REMOVED: return envelope.getRiftPortalRemoved();
            case EXTRACT_STARTED: return envelope.getExtractStarted();
            case EXTRACT_PROGRESS: return envelope.getExtractProgress();
            case EXTRACT_COMPLETED: return envelope.getExtractCompleted();
            case EXTRACT_ABORTED: return envelope.getExtractAborted();
            case EXTRACT_FAILED: return envelope.getExtractFailed();
            case TSY_COLLAPSE_STARTED: return envelope.getTsyCollapseStarted();
            case CONTAINER_STATE: return envelope.getContainerState();
            case SEARCH_STARTED: return envelope.getSearchStarted();
            case SEARCH_PROGRESS: return envelope.getSearchProgress();
            case SEARCH_COMPLETED: return envelope.getSearchCompleted();
            case SEARCH_ABORTED: return envelope.getSearchAborted();
            case SKILL_LV_UP: return envelope.getSkillLvUp();
            case SKILL_CAP_CHANGED: return envelope.getSkillCapChanged();
            case SKILL_SCROLL_USED: return envelope.getSkillScrollUsed();
            case SKILL_SNAPSHOT: return envelope.getSkillSnapshot();
            case FULL_POWER_CHARGING: return envelope.getFullPowerCharging();
            case FULL_POWER_RELEASE: return envelope.getFullPowerRelease();
            case FULL_POWER_EXHAUSTED: return envelope.getFullPowerExhausted();
            case HEALER_NPC_AI_STATE: return envelope.getHealerNpcAiState();
            case YIDAO_HUD_STATE: return envelope.getYidaoHudState();
            case MOVEMENT_STATE: return envelope.getMovementState();
            case SPIRIT_TREASURE_STATE: return envelope.getSpiritTreasureState();
            case SPIRIT_TREASURE_DIALOGUE: return envelope.getSpiritTreasureDialogue();
            case LOOT_CONTAINER_OPEN: return envelope.getLootContainerOpen();
            case LOOT_CONTAINER_UPDATE: return envelope.getLootContainerUpdate();
            case LOOT_CONTAINER_CLOSE: return envelope.getLootContainerClose();
            case FACTION_WAR_STATE: return envelope.getFactionWarState();
            // plan-combat-skill-feedback-bridges-v1 P4：暗器 HUD
            case ANQI_HUD: return envelope.getAnqiHud();
            // plan-combat-skill-feedback-bridges-v1 P5：毒蛊 v2 HUD S2C
            case DUGU_V2_SKILL_CAST: return envelope.getDuguV2SkillCast();
            case DUGU_V2_SELF_CURE: return envelope.getDuguV2SelfCure();
            case DUGU_V2_SHROUD_ACTIVE: return envelope.getDuguV2ShroudActive();
            case PERMANENT_QI_MAX_DECAY_APPLIED: return envelope.getPermanentQiMaxDecayApplied();
            // plan-combat-skill-feedback-bridges-v1 P6：人剑共生 HUD S2C
            case SWORD_BOND_HUD_STATE: return envelope.getSwordBondHudState();
            // plan-exploration-probe-return-v1 P0：神识感知矿脉 S2C
            case MINERAL_PROBE_RESULT: return envelope.getMineralProbeResult();
            // plan-exploration-probe-return-v1 P1：神识感知保鲜 S2C（复用 freshness_update 链）
            case FRESHNESS_UPDATE: return envelope.getFreshnessUpdate();
            // plan-exploration-probe-return-v1 P2：修炼顿悟 S2C
            case INSIGHT_OFFER: return envelope.getInsightOffer();
            case WORKBENCH_OPEN: return envelope.getWorkbenchOpen();
            // 功法 AV 里程碑 系列 2b：zhenmai 5 招专属 HUD S2C
            case ZHENMAI_HUD: return envelope.getZhenmaiHud();
            default: return null;
        }
    }

    /**
     * Returns the set of payload case names that have a type string mapping.
     * Useful for tests.
     */
    static int mappedCaseCount() {
        return CASE_TO_TYPE.size();
    }

    private record OneofVariant(String kind, MessageOrBuilder inner) {}

    @FunctionalInterface
    private interface OneofExtractor<T> {
        OneofVariant extract(T msg);
    }

    private static <T> BridgeResult bridgeOneofFlat(
            T msg, String typeString, OneofExtractor<T> extractor) {
        OneofVariant v = extractor.extract(msg);
        if (v == null) {
            return BridgeResult.error(typeString + " has no oneof variant set");
        }
        try {
            String variantJson = printAndNormalize(v.inner());
            String legacyJson;
            if (variantJson.equals("{}")) {
                legacyJson = "{\"v\":1,\"type\":\"" + typeString
                        + "\",\"kind\":\"" + v.kind() + "\"}";
            } else {
                legacyJson = "{\"v\":1,\"type\":\"" + typeString
                        + "\",\"kind\":\"" + v.kind() + "\"," + variantJson.substring(1);
            }
            return BridgeResult.success(legacyJson);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for "
                    + typeString + "/" + v.kind() + ": " + e.getMessage());
        }
    }

    @FunctionalInterface
    private interface SlotTransformer {
        void transform(JsonObject slotObj);
    }

    private static BridgeResult bridgeSlotConfig(
            MessageOrBuilder msg, String typeString,
            String wrapperField, SlotTransformer slotTransformer) {
        try {
            String raw = printAndNormalize(msg);
            JsonObject root = JsonParser.parseString(raw).getAsJsonObject();
            JsonArray slots = root.has("slots") ? root.getAsJsonArray("slots") : null;
            if (slots != null) {
                for (int i = 0; i < slots.size(); i++) {
                    JsonElement el = slots.get(i);
                    if (!el.isJsonObject()) continue;
                    JsonObject wrapper = el.getAsJsonObject();
                    if (wrapper.size() == 0) {
                        slots.set(i, JsonNull.INSTANCE);
                        continue;
                    }
                    if (wrapper.has(wrapperField) && wrapper.get(wrapperField).isJsonObject()) {
                        JsonObject inner = wrapper.getAsJsonObject(wrapperField);
                        if (slotTransformer != null) {
                            slotTransformer.transform(inner);
                        }
                        slots.set(i, inner);
                    }
                }
            }
            String innerJson = root.toString();
            String legacyJson = "{\"v\":1,\"type\":\"" + typeString + "\"," + innerJson.substring(1);
            return BridgeResult.success(legacyJson);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + typeString + ": " + e.getMessage());
        }
    }

    private static final String[] FORGE_STEP_VARIANTS =
            {"billet", "tempering", "inscription", "consecration"};

    private static BridgeResult bridgeForgeSession(
            MessageOrBuilder msg, String typeString) {
        try {
            String raw = printAndNormalize(msg);
            JsonObject root = JsonParser.parseString(raw).getAsJsonObject();
            if (root.has("step_state") && root.get("step_state").isJsonObject()) {
                JsonObject stepState = root.getAsJsonObject("step_state");
                flattenOneofInPlace(stepState, FORGE_STEP_VARIANTS, "step");
            }
            String innerJson = root.toString();
            String legacyJson = "{\"v\":1,\"type\":\"" + typeString + "\"," + innerJson.substring(1);
            return BridgeResult.success(legacyJson);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + typeString + ": " + e.getMessage());
        }
    }

    private static void flattenOneofInPlace(
            JsonObject obj, String[] variantNames, String tagField) {
        for (String variant : variantNames) {
            if (obj.has(variant) && obj.get(variant).isJsonObject()) {
                JsonObject inner = obj.getAsJsonObject(variant);
                obj.remove(variant);
                obj.addProperty(tagField, variant);
                for (Map.Entry<String, JsonElement> field : inner.entrySet()) {
                    obj.add(field.getKey(), field.getValue());
                }
                return;
            }
        }
        if (obj.has("none_state")) {
            obj.remove("none_state");
            obj.addProperty("step", "none");
            return;
        }
        LOG.warn("flattenOneofInPlace: no known variant matched for tag '{}', keys: {}",
                tagField, obj.keySet());
    }

    private static final String[] SKILL_BAR_VARIANTS = {"item", "skill"};

    private static void flattenSkillBarOneof(JsonObject entryObj) {
        for (String variant : SKILL_BAR_VARIANTS) {
            if (entryObj.has(variant) && entryObj.get(variant).isJsonObject()) {
                JsonObject inner = entryObj.getAsJsonObject(variant);
                entryObj.remove(variant);
                entryObj.addProperty("kind", variant);
                for (Map.Entry<String, JsonElement> field : inner.entrySet()) {
                    entryObj.add(field.getKey(), field.getValue());
                }
                return;
            }
        }
    }

    // ─── inventory_snapshot: unwrap HotbarSlot wrappers ────────────

    private static BridgeResult bridgeInventorySnapshot(
            MessageOrBuilder msg, String typeString) {
        try {
            String raw = printAndNormalize(msg);
            JsonObject root = JsonParser.parseString(raw).getAsJsonObject();
            if (root.has("hotbar") && root.get("hotbar").isJsonArray()) {
                JsonArray hotbar = root.getAsJsonArray("hotbar");
                JsonArray unwrapped = new JsonArray(hotbar.size());
                for (JsonElement slot : hotbar) {
                    if (slot == null || !slot.isJsonObject()) {
                        unwrapped.add(JsonNull.INSTANCE);
                        continue;
                    }
                    JsonObject wrapper = slot.getAsJsonObject();
                    if (wrapper.has("item") && wrapper.get("item").isJsonObject()) {
                        unwrapped.add(wrapper.getAsJsonObject("item"));
                    } else {
                        unwrapped.add(JsonNull.INSTANCE);
                    }
                }
                root.add("hotbar", unwrapped);
            }
            return wrapLegacy(root, typeString);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + typeString + ": " + e.getMessage());
        }
    }

    // ─── movement_state: enum name normalization ────────────────────

    private static BridgeResult bridgeMovementState(
            MessageOrBuilder msg, String typeString) {
        try {
            String raw = printAndNormalize(msg);
            JsonObject root = JsonParser.parseString(raw).getAsJsonObject();
            stripEnumPrefix(root, "movement_action", "MOVEMENT_ACTION_");
            stripEnumPrefix(root, "zone_kind", "MOVEMENT_ZONE_KIND_");
            stripEnumPrefix(root, "rejected_action", "MOVEMENT_ACTION_REQUEST_KIND_");
            return wrapLegacy(root, typeString);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + typeString + ": " + e.getMessage());
        }
    }

    // ─── cast_sync: enum name normalization ─────────────────────────
    //
    // proto3 canonical JSON 把 CastPhase / CastOutcome 打成 proto 枚举值名
    // （CAST_PHASE_CASTING / CAST_OUTCOME_MERIDIAN_GATED）。CastSyncHandler 的
    // phase / outcome switch 期望 serde snake_case（"casting" / "meridian_gated"），
    // 故必须像 movement_state 一样剥前缀转小写，否则 proto 线上整条 cast_sync 静默 noOp。
    // 这也是通用技能警示 HUD 的 reject_* outcome 能到达 client 的前提。

    private static BridgeResult bridgeCastSync(
            MessageOrBuilder msg, String typeString) {
        try {
            String raw = printAndNormalize(msg);
            JsonObject root = JsonParser.parseString(raw).getAsJsonObject();
            stripEnumPrefix(root, "phase", "CAST_PHASE_");
            stripEnumPrefix(root, "outcome", "CAST_OUTCOME_");
            return wrapLegacy(root, typeString);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + typeString + ": " + e.getMessage());
        }
    }

    // ─── skill_config_snapshot: array→map + parse json_config ───────

    private static BridgeResult bridgeSkillConfigSnapshot(
            MessageOrBuilder msg, String typeString) {
        try {
            String raw = printAndNormalize(msg);
            JsonObject root = JsonParser.parseString(raw).getAsJsonObject();
            if (root.has("configs") && root.get("configs").isJsonArray()) {
                JsonArray arr = root.getAsJsonArray("configs");
                JsonObject map = new JsonObject();
                for (JsonElement el : arr) {
                    if (el == null || !el.isJsonObject()) continue;
                    JsonObject entry = el.getAsJsonObject();
                    String skillId = entry.has("skill_id")
                            ? entry.get("skill_id").getAsString() : null;
                    if (skillId == null || skillId.isEmpty()) continue;
                    String jsonConfig = entry.has("json_config")
                            ? entry.get("json_config").getAsString() : "{}";
                    try {
                        map.add(skillId, JsonParser.parseString(jsonConfig));
                    } catch (Exception ignored) {
                        map.add(skillId, new JsonObject());
                    }
                }
                root.add("configs", map);
            }
            return wrapLegacy(root, typeString);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + typeString + ": " + e.getMessage());
        }
    }

    // ─── skill_snapshot: array→map + unwrap entry ───────────────────

    private static BridgeResult bridgeSkillSnapshot(
            MessageOrBuilder msg, String typeString) {
        try {
            String raw = printAndNormalize(msg);
            JsonObject root = JsonParser.parseString(raw).getAsJsonObject();
            if (root.has("skills") && root.get("skills").isJsonArray()) {
                JsonArray arr = root.getAsJsonArray("skills");
                JsonObject map = new JsonObject();
                for (JsonElement el : arr) {
                    if (el == null || !el.isJsonObject()) continue;
                    JsonObject wrapper = el.getAsJsonObject();
                    String skillName = wrapper.has("skill_name")
                            ? wrapper.get("skill_name").getAsString() : null;
                    if (skillName == null || skillName.isEmpty()) continue;
                    JsonObject entry = wrapper.has("entry") && wrapper.get("entry").isJsonObject()
                            ? wrapper.getAsJsonObject("entry") : new JsonObject();
                    map.add(skillName, entry);
                }
                root.add("skills", map);
            }
            return wrapLegacy(root, typeString);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + typeString + ": " + e.getMessage());
        }
    }

    // ─── cultivation_detail: AoS→SoA + enum normalization ──────────

    private static final Map<String, Integer> MERIDIAN_ID_TO_INDEX;
    static {
        Map<String, Integer> m = new HashMap<>();
        m.put("MERIDIAN_ID_LUNG", 0);
        m.put("MERIDIAN_ID_LARGE_INTESTINE", 1);
        m.put("MERIDIAN_ID_STOMACH", 2);
        m.put("MERIDIAN_ID_SPLEEN", 3);
        m.put("MERIDIAN_ID_HEART", 4);
        m.put("MERIDIAN_ID_SMALL_INTESTINE", 5);
        m.put("MERIDIAN_ID_BLADDER", 6);
        m.put("MERIDIAN_ID_KIDNEY", 7);
        m.put("MERIDIAN_ID_PERICARDIUM", 8);
        m.put("MERIDIAN_ID_TRIPLE_ENERGIZER", 9);
        m.put("MERIDIAN_ID_GALLBLADDER", 10);
        m.put("MERIDIAN_ID_LIVER", 11);
        m.put("MERIDIAN_ID_REN", 12);
        m.put("MERIDIAN_ID_DU", 13);
        m.put("MERIDIAN_ID_CHONG", 14);
        m.put("MERIDIAN_ID_DAI", 15);
        m.put("MERIDIAN_ID_YIN_QIAO", 16);
        m.put("MERIDIAN_ID_YANG_QIAO", 17);
        m.put("MERIDIAN_ID_YIN_WEI", 18);
        m.put("MERIDIAN_ID_YANG_WEI", 19);
        MERIDIAN_ID_TO_INDEX = Map.copyOf(m);
    }

    private static BridgeResult bridgeCultivationDetail(
            MessageOrBuilder msg, String typeString) {
        try {
            String raw = printAndNormalize(msg);
            JsonObject root = JsonParser.parseString(raw).getAsJsonObject();

            if (root.has("meridians") && root.get("meridians").isJsonArray()) {
                JsonArray meridians = root.getAsJsonArray("meridians");
                int size = 20;
                JsonArray opened = initArray(size, false);
                JsonArray flowRate = initDoubleArray(size);
                JsonArray flowCapacity = initDoubleArray(size);
                JsonArray integrity = initDoubleArray(size);
                JsonArray openProgress = initDoubleArray(size);
                JsonArray cracksCount = initArray(size, 0);

                for (JsonElement el : meridians) {
                    if (el == null || !el.isJsonObject()) continue;
                    JsonObject m = el.getAsJsonObject();
                    String idStr = m.has("id") ? m.get("id").getAsString() : null;
                    Integer idx = idStr != null ? MERIDIAN_ID_TO_INDEX.get(idStr) : null;
                    if (idx == null) continue;
                    if (m.has("opened")) opened.set(idx, m.get("opened"));
                    if (m.has("flow_rate")) flowRate.set(idx, m.get("flow_rate"));
                    if (m.has("flow_capacity")) flowCapacity.set(idx, m.get("flow_capacity"));
                    if (m.has("integrity")) integrity.set(idx, m.get("integrity"));
                    if (m.has("open_progress")) openProgress.set(idx, m.get("open_progress"));
                    if (m.has("cracks_count")) cracksCount.set(idx, m.get("cracks_count"));
                }

                root.remove("meridians");
                root.add("opened", opened);
                root.add("flow_rate", flowRate);
                root.add("flow_capacity", flowCapacity);
                root.add("integrity", integrity);
                root.add("open_progress", openProgress);
                root.add("cracks_count", cracksCount);
            }

            normalizeRealmField(root, "realm");

            if (root.has("target_meridian") && root.get("target_meridian").isJsonPrimitive()
                    && root.get("target_meridian").getAsJsonPrimitive().isString()) {
                Integer idx = MERIDIAN_ID_TO_INDEX.get(root.get("target_meridian").getAsString());
                if (idx != null) {
                    root.addProperty("target_meridian", idx);
                } else {
                    root.remove("target_meridian");
                }
            }

            stripEnumPrefix(root, "qi_color_main", "COLOR_KIND_");
            stripEnumPrefix(root, "qi_color_secondary", "COLOR_KIND_");

            if (root.has("practice_weights") && root.get("practice_weights").isJsonArray()) {
                for (JsonElement el : root.getAsJsonArray("practice_weights")) {
                    if (el != null && el.isJsonObject()) {
                        stripEnumPrefix(el.getAsJsonObject(), "color", "COLOR_KIND_");
                    }
                }
            }

            return wrapLegacy(root, typeString);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + typeString + ": " + e.getMessage());
        }
    }

    // ─── craft_recipe_list: material/output tuples + enum ────────────

    private static BridgeResult bridgeCraftRecipeList(
            MessageOrBuilder msg, String typeString) {
        try {
            String raw = printAndNormalize(msg);
            JsonObject root = JsonParser.parseString(raw).getAsJsonObject();
            if (root.has("recipes") && root.get("recipes").isJsonArray()) {
                for (JsonElement recipeEl : root.getAsJsonArray("recipes")) {
                    if (recipeEl == null || !recipeEl.isJsonObject()) continue;
                    JsonObject recipe = recipeEl.getAsJsonObject();
                    stripEnumPrefix(recipe, "category", "CRAFT_CATEGORY_");
                    convertPairArrayToTuples(recipe, "materials");
                    convertPairToTuple(recipe, "output");
                    if (recipe.has("requirements") && recipe.get("requirements").isJsonObject()) {
                        JsonObject req = recipe.getAsJsonObject("requirements");
                        normalizeRealmField(req, "realm_min");
                        convertPairToTuple(req, "qi_color_min");
                        if (req.has("qi_color_min") && req.get("qi_color_min").isJsonArray()) {
                            JsonArray qcArr = req.getAsJsonArray("qi_color_min");
                            if (qcArr.size() >= 1 && qcArr.get(0).isJsonPrimitive()) {
                                String colorVal = qcArr.get(0).getAsString();
                                if (colorVal.startsWith("COLOR_KIND_")) {
                                    qcArr.set(0, new JsonPrimitive(
                                            colorVal.substring("COLOR_KIND_".length())
                                                    .toLowerCase(Locale.ROOT)));
                                }
                            }
                        }
                    }
                }
            }
            return wrapLegacy(root, typeString);
        } catch (com.google.protobuf.InvalidProtocolBufferException e) {
            return BridgeResult.error("proto→JSON conversion failed for " + typeString + ": " + e.getMessage());
        }
    }

    private static void convertPairToTuple(JsonObject parent, String field) {
        if (!parent.has(field) || !parent.get(field).isJsonObject()) return;
        JsonObject obj = parent.getAsJsonObject(field);
        JsonArray tuple = new JsonArray(2);
        tuple.add(obj.has("template_id") ? obj.get("template_id").getAsString() : "");
        tuple.add(obj.has("count") ? obj.get("count").getAsInt() : 0);
        // qi_color_min uses "color" + "min_share" instead
        if (!obj.has("template_id") && obj.has("color")) {
            tuple = new JsonArray(2);
            tuple.add(obj.get("color").getAsString());
            tuple.add(obj.has("min_share") ? obj.get("min_share").getAsFloat() : 0f);
        }
        parent.add(field, tuple);
    }

    private static void convertPairArrayToTuples(JsonObject parent, String field) {
        if (!parent.has(field) || !parent.get(field).isJsonArray()) return;
        JsonArray arr = parent.getAsJsonArray(field);
        JsonArray tuples = new JsonArray(arr.size());
        for (JsonElement el : arr) {
            if (el == null || !el.isJsonObject()) {
                tuples.add(el);
                continue;
            }
            JsonObject obj = el.getAsJsonObject();
            JsonArray tuple = new JsonArray(2);
            tuple.add(obj.has("template_id") ? obj.get("template_id").getAsString() : "");
            tuple.add(obj.has("count") ? obj.get("count").getAsInt() : 0);
            tuples.add(tuple);
        }
        parent.add(field, tuples);
    }

    // ─── Shared helpers ─────────────────────────────────────────────

    private static BridgeResult wrapLegacy(JsonObject root, String typeString) {
        String innerJson = root.toString();
        String legacyJson;
        if (innerJson.equals("{}")) {
            legacyJson = "{\"v\":1,\"type\":\"" + typeString + "\"}";
        } else {
            legacyJson = "{\"v\":1,\"type\":\"" + typeString + "\"," + innerJson.substring(1);
        }
        return BridgeResult.success(legacyJson);
    }

    private static void stripEnumPrefix(JsonObject obj, String field, String prefix) {
        if (!obj.has(field) || !obj.get(field).isJsonPrimitive()) return;
        String value = obj.get(field).getAsString();
        if (value.startsWith(prefix)) {
            obj.addProperty(field, value.substring(prefix.length()).toLowerCase(Locale.ROOT));
        }
    }

    private static void normalizeRealmField(JsonObject obj, String field) {
        if (!obj.has(field) || !obj.get(field).isJsonPrimitive()) return;
        String value = obj.get(field).getAsString();
        if (value.startsWith("REALM_")) {
            String suffix = value.substring("REALM_".length());
            obj.addProperty(field,
                    suffix.substring(0, 1).toUpperCase(Locale.ROOT)
                    + suffix.substring(1).toLowerCase(Locale.ROOT));
        }
    }

    private static JsonArray initDoubleArray(int size) {
        JsonArray arr = new JsonArray(size);
        for (int i = 0; i < size; i++) arr.add(0.0);
        return arr;
    }

    private static JsonArray initArray(int size, Object defaultVal) {
        JsonArray arr = new JsonArray(size);
        for (int i = 0; i < size; i++) {
            if (defaultVal instanceof Boolean b) arr.add(b);
            else if (defaultVal instanceof Number n) arr.add(n);
            else arr.add(JsonNull.INSTANCE);
        }
        return arr;
    }

    private static final Pattern INT64_STRING = Pattern.compile("-?\\d{1,20}");

    private static String printAndNormalize(MessageOrBuilder msg)
            throws com.google.protobuf.InvalidProtocolBufferException {
        String raw = PRINTER.print(msg);
        if (raw.equals("{}")) return raw;
        JsonElement tree = JsonParser.parseString(raw);
        normalizeNumericStrings(tree);
        return tree.toString();
    }

    private static void normalizeNumericStrings(JsonElement element) {
        if (element.isJsonObject()) {
            JsonObject obj = element.getAsJsonObject();
            for (Map.Entry<String, JsonElement> e : obj.entrySet()) {
                JsonElement val = e.getValue();
                if (val.isJsonPrimitive() && val.getAsJsonPrimitive().isString()) {
                    String s = val.getAsJsonPrimitive().getAsString();
                    if (INT64_STRING.matcher(s).matches()) {
                        try {
                            e.setValue(new JsonPrimitive(Long.parseLong(s)));
                        } catch (NumberFormatException ignored) {
                        }
                    }
                } else {
                    normalizeNumericStrings(val);
                }
            }
        } else if (element.isJsonArray()) {
            JsonArray arr = element.getAsJsonArray();
            for (int i = 0; i < arr.size(); i++) {
                JsonElement val = arr.get(i);
                if (val.isJsonPrimitive() && val.getAsJsonPrimitive().isString()) {
                    String s = val.getAsJsonPrimitive().getAsString();
                    if (INT64_STRING.matcher(s).matches()) {
                        try {
                            arr.set(i, new JsonPrimitive(Long.parseLong(s)));
                        } catch (NumberFormatException ignored) {
                        }
                    }
                } else {
                    normalizeNumericStrings(val);
                }
            }
        }
    }
}
