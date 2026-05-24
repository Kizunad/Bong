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

    private static final Pattern INT64_STRING = Pattern.compile("-?\\d{1,20}");

    /**
     * Print proto message to JSON and convert proto3 string-encoded int64/uint64
     * back to JSON numbers so legacy handlers' {@code isNumber()} checks pass.
     */
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
                            // overflow — keep as string
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
                            // overflow — keep as string
                        }
                    }
                } else {
                    normalizeNumericStrings(val);
                }
            }
        }
    }
}
