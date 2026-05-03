package com.bong.client.network;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ServerDataRouterTest {
    @Test
    void defaultRouterRegistersAllExpectedTypes() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        assertEquals(Set.of(
            "welcome",
            "heartbeat",
            "narration",
            "zone_info",
            "event_alert",
            "player_state",
            "ui_open",
            "cultivation_detail",
            "inventory_snapshot",
            "inventory_event",
            "dropped_loot_sync",
            // Botany handlers (plan-botany-v1 §4).
            "botany_harvest_progress",
            "botany_plant_v2_render_profiles",
            "botany_skill",
            // Combat UI handlers (plan-combat-ui §U1–U7).
            "combat_event",
            "status_snapshot",
            "derived_attrs_sync",
            "death_screen",
            "terminate_screen",
            "wounds_snapshot",
            "tribulation_state",
            "tribulation_broadcast",
            "ascension_quota",
            "heart_demon_offer",
            // Alchemy handlers (plan-alchemy-v1 §4).
            "alchemy_furnace",
            "alchemy_session",
            "alchemy_outcome_forecast",
            "alchemy_recipe_book",
            "alchemy_contamination",
            "alchemy_outcome_resolved",
            // HUD state push (plan-HUD-v1 §11.4).
            "combat_hud_state",
            "defense_window",
            "cast_sync",
            "quickslot_config",
            "skillbar_config",
            "techniques_snapshot",
            "unlocks_sync",
            "event_stream_push",
            "burst_meridian_event",
            // plan-weapon-v1 §8.2 装备/损坏推送。
            "weapon_equipped",
            "weapon_broken",
            "treasure_equipped",
            // plan-woliu-v1 §A.1 涡流 HUD 状态推送。
            "vortex_state",
            // plan-perception-v1.1 §4 server-authoritative vision/sense push.
            "realm_vision_params",
            "spiritual_sense_targets",
            // plan-tsy-extract-v1 §4.1 撤离点 / 撤离进度 HUD 推送。
            "rift_portal_state",
            "rift_portal_removed",
            "extract_started",
            "extract_progress",
            "extract_completed",
            "extract_aborted",
            "extract_failed",
            "tsy_collapse_started_ipc",
            // plan-input-binding-v1 §4 — TSY 容器搜刮 / 搜刮 HUD 推送。
            "container_state",
            "search_started",
            "search_progress",
            "search_completed",
            "search_aborted",
            // plan-lingtian-v1 §4 active session 推送。
            "lingtian_session",
            // plan-skill-v1 §8 子技能 IPC（4 条 server→client channel 镜像）。
            "skill_xp_gain",
            "skill_lv_up",
            "skill_cap_changed",
            "skill_scroll_used",
            "skill_snapshot",
            // plan-forge-v1 §4 — 炼器（武器）
            "forge_station",
            "forge_session",
            "forge_outcome",
            "forge_blueprint_book",
            // plan-social-v1 §7 — 匿名 / 暴露 / 关系 / 声名 / 切磋邀请。
            "social_anonymity",
            "social_exposure",
            "social_pact",
            "social_feud",
            "social_renown_delta",
            "sparring_invite"
        ), router.registeredTypes());
    }

    @Test
    void routesLegacyWelcomeWithMessageDispatch() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-legacy-welcome.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertFalse(result.isNoOp());
        assertEquals("welcome", result.envelope().type());
        assertEquals("Bong server connected", result.dispatch().legacyMessage().orElseThrow());
    }

    @Test
    void routesLegacyHeartbeatWithMessageDispatch() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-legacy-heartbeat.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertFalse(result.isNoOp());
        assertEquals("heartbeat", result.envelope().type());
        assertEquals("mock agent tick", result.dispatch().legacyMessage().orElseThrow());
    }

    @Test
    void routesNestedNarrationWithNarrationUpdate() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-nested-narration.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertTrue(result.dispatch().legacyMessage().isEmpty());
        assertEquals(2, result.dispatch().chatMessages().size());
        assertTrue(result.dispatch().narrationState().isPresent());
        assertTrue(result.dispatch().toastNarrationState().isPresent());
        assertTrue(result.logMessage().contains("narration"));
    }

    @Test
    void routesZoneInfoWithZoneStateDispatch() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-zone-info.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertTrue(result.dispatch().zoneState().isPresent());
        assertEquals("blood_valley", result.dispatch().zoneState().orElseThrow().zoneId());
    }

    @Test
    void routesEventAlertWithToastAndOptionalEffectDispatch() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-event-alert-critical.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertTrue(result.dispatch().alertToast().isPresent());
        assertTrue(result.dispatch().visualEffectState().isPresent());
    }

    @Test
    void routesBotanyHarvestProgressIntoStoreHandler() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-botany-harvest-progress.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertEquals("botany_harvest_progress", result.envelope().type());
    }

    @Test
    void routesBotanySkillIntoStoreHandler() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-botany-skill.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertEquals("botany_skill", result.envelope().type());
    }

    @Test
    void routesBurstMeridianEventWithoutNoOp() {
        String json = """
            {"v":1,"type":"burst_meridian_event","skill":"thunder_step","caster":"offline:Azure","target":"npc_1v1","tick":84000,"overload_ratio":0.75,"integrity_snapshot":0.4}
            """;
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertFalse(result.isNoOp());
        assertEquals("burst_meridian_event", result.envelope().type());
    }

    @Test
    void unknownTypeBecomesSafeNoOp() throws IOException {
        String json = PayloadFixtureLoader.readText("unknown-type.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertFalse(result.isHandled());
        assertTrue(result.isNoOp());
        assertEquals("mystery_signal", result.envelope().type());
        assertTrue(result.logMessage().contains("No registered handler"));
    }

    @Test
    void malformedJsonReturnsParseErrorInsteadOfThrowing() throws IOException {
        String json = PayloadFixtureLoader.readText("malformed-event-alert.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isParseError());
        assertNull(result.dispatch());
        assertTrue(result.logMessage().contains("Malformed JSON"));
    }

    @Test
    void unsupportedVersionReturnsParseErrorInsteadOfThrowing() throws IOException {
        String json = PayloadFixtureLoader.readText("wrong-version-player-state.json");
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isParseError());
        assertNull(result.dispatch());
        assertTrue(result.logMessage().contains("Unsupported version"));
    }
}
