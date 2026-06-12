package com.bong.client.network;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
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
            "qi_color_observed",
            "inventory_snapshot",
            "inventory_event",
            "dropped_loot_sync",
            // Botany handlers (plan-botany-v1 §4).
            "botany_harvest_progress",
            "gathering_session",
            "mining_progress",
            "lumber_progress",
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
            "breakthrough_cinematic",
            // plan-knockback-physics-v1 — 击退同步复用 hit_pushback 视觉通道。
            "knockback_sync",
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
            "skill_config_snapshot",
            "unlocks_sync",
            "event_stream_push",
            "burst_meridian_event",
            // plan-baomai-v2 P2 — 爆脉全力一击蓄力 / 释放 / 虚脱 HUD 推送。
            "full_power_charging_state",
            "full_power_release",
            "full_power_exhausted_state",
            // plan-weapon-v1 §8.2 装备/损坏推送。
            "weapon_equipped",
            "weapon_broken",
            "treasure_equipped",
            // plan-shield-block-v1 §P3 破盾事件 / §P4 格挡命中事件。
            "shield_broken",
            "shield_block_hit",
            // plan-woliu-v1 §A.1 涡流 HUD 状态推送。
            "vortex_state",
            // plan-dugu-v1 P0/P1 — 毒蛊受毒状态 HUD 推送。
            "dugu_poison_state",
            // plan-poison-trait-v1 — 毒性真元 HUD 与服丹事件推送。
            "poison_dose_event",
            "poison_overdose_event",
            "poison_trait_state",
            // plan-anqi-v1 P1 暗器载体 HUD 状态推送。
            "carrier_state",
            // plan-tuike-v1 §P0 伪皮 HUD 状态推送。
            "false_skin_state",
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
            // plan-lingtian-process-v1 P3 — 加工进度 / freshness UI tag 推送。
            "processing_session",
            "freshness_update",
            // plan-yidao-v1 — 医者 NPC AI / 医道 HUD 状态推送。
            "healer_npc_ai_state",
            "yidao_hud_state",
            // plan-movement-v1 — movement HUD/action state push.
            "movement_state",
            // plan-spirit-treasure-v1 — 灵宝状态与器灵对话推送。
            "spirit_treasure_state",
            "spirit_treasure_dialogue",
            // plan-coffin-v1 — 卧棺状态与寿命倍率推送。
            "coffin_state",
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
            "niche_intrusion",
            "niche_guardian_fatigue",
            "niche_guardian_broken",
            "sparring_invite",
            "trade_offer",
            // plan-identity-v1 P5 — 身份面板 / HUD 当前 identity 状态。
            "identity_panel_state",
            // plan-craft-v1 P2 — 通用手搓 IPC（4 类）
            "craft_recipe_list",
            "craft_session_state",
            "craft_outcome",
            "recipe_unlocked",
            // plan-workbench-recipes-v1 P3.1 — 制作台 UI 打开
            "workbench_open",
            // plan-supply-coffin-loot-ui P1 — 外部容器（物资棺搜刮 UI）
            "loot_container_open",
            "loot_container_update",
            "loot_container_close",
            // plan-offscreen-war-v1 P9 — 战事 HUD 状态
            "faction_war_state",
            // plan-combat-skill-feedback-bridges-v1 P4 — 暗器 HUD 状态
            "anqi_hud",
            // plan-combat-skill-feedback-bridges-v1 P5 — 毒蛊 v2 HUD S2C
            "dugu_v2_skill_cast",
            "dugu_v2_self_cure",
            "dugu_v2_shroud_active",
            "permanent_qi_max_decay_applied",
            // plan-combat-skill-feedback-bridges-v1 P6 — 人剑共生 HUD S2C
            "sword_bond_hud_state",
            // plan-exploration-probe-return-v1 P0 — 神识感知矿脉回执 S2C
            "mineral_probe_result",
            // plan-exploration-probe-return-v1 P2 — 修炼顿悟邀约 S2C
            "insight_offer",
            // plan-agent-ui-data-v1 P1 — 天道动态 UI 面板 S2C（request + close 同 handler）
            "agent_ui_request",
            "agent_ui_close"
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
    void routesGatheringSessionIntoStoreHandler() {
        String json = """
            {"v":1,"type":"gathering_session","session_id":"mine-1","progress_ticks":30,"total_ticks":60,"target_name":"凡铁矿","target_type":"ore","quality_hint":"fine_likely","tool_used":"pickaxe_iron","interrupted":false,"completed":false}
            """;
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertEquals("gathering_session", result.envelope().type());
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

    // ---- plan-shield-block-v1 §P3 e2e: shield_broken 端到端路由验证 ------

    /**
     * e2e：server 推送 shield_broken payload → client router 路由 → 收到 toast「盾已碎裂」+
     * SHIELD_BREAK_FLASH VisualEffect。
     * 验证的是完整链路：ServerDataRouter.createDefault() → ShieldBrokenHandler → dispatch。
     */
    @Test
    void e2e_shieldBroken_woodenShield_routesToHandlerWithToastAndFlash() {
        com.bong.client.combat.EquippedShieldStore.resetForTests();
        String json = "{\"v\":1,\"type\":\"shield_broken\",\"instance_id\":77,\"template_id\":\"wooden_shield\"}";
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(),
            "shield_broken payload 不应解析失败");
        assertTrue(result.isHandled(),
            "shield_broken 应被路由并标记为 handled（找到注册的 ShieldBrokenHandler）");
        assertFalse(result.isNoOp(),
            "shield_broken 不应变成 noOp（noOp 意味着没有注册 handler）");
        assertEquals("shield_broken", result.envelope().type());

        // toast 文案验证：「盾已碎裂」
        assertTrue(result.dispatch().alertToast().isPresent(),
            "e2e：shield_broken dispatch 应包含 alertToast");
        assertEquals("盾已碎裂", result.dispatch().alertToast().orElseThrow().text(),
            "e2e toast 文案应为「盾已碎裂」");

        // VisualEffect SHIELD_BREAK_FLASH
        assertTrue(result.dispatch().visualEffectState().isPresent(),
            "e2e：shield_broken dispatch 应包含 visualEffectState");
        assertSame(
            com.bong.client.state.VisualEffectState.EffectType.SHIELD_BREAK_FLASH,
            result.dispatch().visualEffectState().orElseThrow().effectType(),
            "e2e：visualEffect 类型应为 SHIELD_BREAK_FLASH，"
                + "实际: " + result.dispatch().visualEffectState().orElseThrow().effectType()
        );
    }

    @Test
    void e2e_shieldBroken_boneShield_routesToHandlerWithToast() {
        com.bong.client.combat.EquippedShieldStore.resetForTests();
        String json = "{\"v\":1,\"type\":\"shield_broken\",\"instance_id\":88,\"template_id\":\"bone_shield\"}";
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertEquals("盾已碎裂", result.dispatch().alertToast().orElseThrow().text(),
            "骨盾破碎 e2e toast 应为「盾已碎裂」");
    }

    /**
     * e2e：连续格挡至破盾 payload → EquippedShieldStore 清空（off_hand 盾槽消失）。
     */
    @Test
    void e2e_shieldBroken_clearsOffHandShieldStore() {
        long instanceId = 55L;
        com.bong.client.combat.EquippedShieldStore.equip(
            new com.bong.client.combat.EquippedShield(instanceId, "wooden_shield", 0f, 100f)
        );
        // precondition: 盾槽有盾
        assertNotNull(com.bong.client.combat.EquippedShieldStore.snapshot(),
            "e2e 前置：盾槽应有盾");

        String json = "{\"v\":1,\"type\":\"shield_broken\",\"instance_id\":" + instanceId
            + ",\"template_id\":\"wooden_shield\"}";
        ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertNull(com.bong.client.combat.EquippedShieldStore.snapshot(),
            "e2e：破盾后 off_hand 盾槽应清空");
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
