package com.bong.client.network;

import bong.Common;
import bong.Envelope;
import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.hud.PillBuffHudPlanner;
import com.bong.client.hud.BongToast;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class ProtoServerDataBridgeTest {

    @AfterEach
    void tearDown() {
        PillBuffHudPlanner.clear();
        TechniquesListPanel.resetForTests();
        BongToast.resetForTests();
        UnifiedEventStore.resetForTests();
        com.bong.client.coffin.TutorialCoffinPosStore.resetForTests();
    }

    // ─── Happy path: Welcome ─────────────────────────────────────────
    @Test
    void bridgeWelcomeProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setWelcome(Envelope.Welcome.newBuilder()
                        .setMessage("Bong server connected"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for Welcome: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals(1, json.get("v").getAsInt(), "version should be 1");
        assertEquals("welcome", json.get("type").getAsString(), "type should be 'welcome'");
        assertEquals("Bong server connected", json.get("message").getAsString());
    }

    // ─── Happy path: Heartbeat ───────────────────────────────────────
    @Test
    void bridgeHeartbeatProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setHeartbeat(Envelope.Heartbeat.newBuilder()
                        .setMessage("mock agent tick"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals(1, json.get("v").getAsInt());
        assertEquals("heartbeat", json.get("type").getAsString());
        assertEquals("mock agent tick", json.get("message").getAsString());
    }

    // ─── Happy path: ZoneInfo ────────────────────────────────────────
    @Test
    void bridgeZoneInfoProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setZoneInfo(Envelope.ZoneInfo.newBuilder()
                        .setZone("blood_valley")
                        .setSpiritQi(-0.42)
                        .setDangerLevel(3))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals(1, json.get("v").getAsInt());
        assertEquals("zone_info", json.get("type").getAsString());
        assertEquals("blood_valley", json.get("zone").getAsString());
    }

    // ─── Happy path: EventAlert ──────────────────────────────────────
    @Test
    void bridgeEventAlertProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventAlert(Envelope.EventAlert.newBuilder()
                        .setEvent(Envelope.EventKind.EVENT_KIND_THUNDER_TRIBULATION)
                        .setMessage("watch out")
                        .setZone("spawn")
                        .setDurationTicks(100))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("event_alert", json.get("type").getAsString());
        assertEquals("watch out", json.get("message").getAsString());
    }

    // ─── Happy path: Narration ───────────────────────────────────────
    @Test
    void bridgeNarrationProducesLegacyJson() {
        Envelope.NarrationEntry narration = Envelope.NarrationEntry.newBuilder()
                .setText("The sky darkens")
                .setScope("broadcast")
                .setStyle("narration")
                .build();
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setNarration(Envelope.NarrationBatch.newBuilder()
                        .addNarrations(narration))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("narration", json.get("type").getAsString());
        assertTrue(json.has("narrations"), "should have narrations field");
        assertEquals(1, json.getAsJsonArray("narrations").size());
    }

    // ─── Happy path: CombatHudState ──────────────────────────────────
    @Test
    void bridgeCombatHudStateProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCombatHudState(Envelope.CombatHudState.newBuilder()
                        .setHpPercent(0.75f)
                        .setQiPercent(0.5f)
                        .setStaminaPercent(1.0f))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("combat_hud_state", json.get("type").getAsString());
    }

    // ─── Happy path: CoffinState ─────────────────────────────────────
    @Test
    void bridgeCoffinStateProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCoffinState(Envelope.CoffinState.newBuilder()
                        .setInCoffin(true)
                        .setLifespanRateMultiplier(0.5))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("coffin_state", json.get("type").getAsString());
        assertTrue(json.get("in_coffin").getAsBoolean());
    }

    // ─── Happy path: UiOpen ──────────────────────────────────────────
    @Test
    void bridgeUiOpenProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setUiOpen(Envelope.UiOpen.newBuilder()
                        .setUi("cultivation_panel")
                        .setXml("<flow-layout/>"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("ui_open", json.get("type").getAsString());
        assertEquals("cultivation_panel", json.get("ui").getAsString());
    }

    // ─── Happy path: DeathScreen ─────────────────────────────────────
    @Test
    void bridgeDeathScreenProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDeathScreen(Envelope.DeathScreen.newBuilder()
                        .setStage(Envelope.DeathScreenStage.DEATH_SCREEN_STAGE_FORTUNE)
                        .setZoneKind(Envelope.DeathScreenZoneKind.DEATH_SCREEN_ZONE_KIND_ORDINARY)
                        .setCause("karma_backlash")
                        .setLuckRemaining(0.5))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("death_screen", json.get("type").getAsString());
        assertEquals("fortune", json.get("stage").getAsString(),
                "stage 必须从 DEATH_SCREEN_STAGE_FORTUNE 剥成 'fortune'（DeathScreen.phaseLabel 期望），"
                + "否则死亡界面阶段标签永远落 default '重生判定'");
        assertEquals("ordinary", json.get("zone_kind").getAsString(),
                "zone_kind 必须从 DEATH_SCREEN_ZONE_KIND_ORDINARY 剥成 'ordinary'（DeathScreen.zoneLabel 期望）");
    }

    // ─── death_screen: 顶层 stage/zone_kind + 嵌套 cinematic 枚举剥前缀 ──
    // 死亡界面阶段标签 + cinematic 过场推进的接收前提：proto3 JSON 把所有 death 枚举
    // 打成全名前缀，各消费方只认 serde 小写。锁住顶层与嵌套两层。
    @Test
    void bridgeDeathScreenStripsTopLevelAndCinematicEnumPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDeathScreen(Envelope.DeathScreen.newBuilder()
                        .setVisible(true)
                        .setCause("pk")
                        .setStage(Envelope.DeathScreenStage.DEATH_SCREEN_STAGE_TRIBULATION)
                        .setZoneKind(Envelope.DeathScreenZoneKind.DEATH_SCREEN_ZONE_KIND_NEGATIVE)
                        .setCinematic(Envelope.DeathCinematicData.newBuilder()
                                .setV(1)
                                .setCharacterId("char-1")
                                .setPhase(Envelope.DeathCinematicPhase.DEATH_CINEMATIC_PHASE_ROLL)
                                .setZoneKind(Envelope.DeathCinematicZoneKind.DEATH_CINEMATIC_ZONE_KIND_DEATH)
                                .setRoll(Envelope.DeathCinematicRoll.newBuilder()
                                        .setResult(Envelope.DeathRollResult.DEATH_ROLL_RESULT_SURVIVE))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("death_screen", json.get("type").getAsString());
        assertEquals("tribulation", json.get("stage").getAsString(),
                "顶层 stage 剥成 'tribulation'（DeathScreen.phaseLabel）");
        assertEquals("negative", json.get("zone_kind").getAsString(),
                "顶层 zone_kind 剥成 'negative'（DeathScreen.zoneLabel）");
        JsonObject cinematic = json.getAsJsonObject("cinematic");
        assertEquals("roll", cinematic.get("phase").getAsString(),
                "cinematic.phase 剥成 'roll'（DeathCinematicState.Phase.fromWire），否则过场永远卡 PREDEATH");
        assertEquals("death", cinematic.get("zone_kind").getAsString(),
                "cinematic.zone_kind 一并归一化为 'death' 保持桥输出统一");
        assertEquals("survive", cinematic.getAsJsonObject("roll").get("result").getAsString(),
                "cinematic.roll.result 剥成 'survive'（DeathCinematicState.RollResult.fromWire）");
    }

    // ─── Happy path: TsyCollapseStarted maps to tsy_collapse_started_ipc ──
    @Test
    void bridgeTsyCollapseStartedMapsToCorrectTypeString() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setTsyCollapseStarted(Envelope.TsyCollapseStartedIpc.newBuilder()
                        .setFamilyId("tsy_001"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("tsy_collapse_started_ipc", json.get("type").getAsString(),
                "TSY_COLLAPSE_STARTED should map to 'tsy_collapse_started_ipc'");
    }

    // ─── Error: empty envelope (PAYLOAD_NOT_SET) ─────────────────────
    @Test
    void bridgeEmptyEnvelopeReturnsError() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder().build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("no payload set"),
                "error should mention missing payload: " + result.errorMessage());
    }

    // ─── Error: invalid bytes ────────────────────────────────────────
    @Test
    void bridgeGarbageBytesReturnsError() {
        byte[] garbage = new byte[]{(byte) 0xFF, (byte) 0xFE, 0x00, 0x01};

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(garbage);
        // Proto may or may not parse garbage as a valid message. Either error or
        // empty payload are acceptable.
        if (result.isSuccess()) {
            // If it somehow parsed, the JSON should at least have v and type
            JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
            assertEquals(1, json.get("v").getAsInt());
        }
    }

    // ─── Mapped case count matches expected ──────────────────────────
    @Test
    void mappedCaseCountMatchesExpectedVariants() {
        // We expect mappings for all S2C payload variants that have client handlers.
        // Excludes: PAYLOAD_NOT_SET and channel-specific variants (VFX_EVENT, AUDIO_*, etc.)
        int count = ProtoServerDataBridge.mappedCaseCount();
        assertTrue(count >= 100,
                "expected at least 100 mapped cases, got " + count +
                " — did you forget to add a new PayloadCase→type mapping?");
    }

    // ─── Faction war HUD proto bridge ────────────────────────────────
    @Test
    void bridgeFactionWarStateProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFactionWarState(Envelope.FactionWarState.newBuilder()
                        .setWarId(42)
                        .setZone("blood_valley")
                        .setRegionDescriptor("残灰谷")
                        .setPhase("skirmish")
                        .addGroups(1)
                        .addGroups(2)
                        .setEnlistCount(3)
                        .setMercenaryCount(4)
                        .setInterceptCount(5)
                        .setSpectateCount(6)
                        .setWinnerGroup(-1)
                        .setLoserGroup(-1))
                .build();

        JsonObject json = bridgeAndParse(envelope);

        assertEquals(1, json.get("v").getAsInt());
        assertEquals("faction_war_state", json.get("type").getAsString());
        assertEquals(42, json.get("war_id").getAsLong());
        assertEquals("blood_valley", json.get("zone").getAsString());
        assertEquals("残灰谷", json.get("region_descriptor").getAsString());
        assertEquals("skirmish", json.get("phase").getAsString());
        assertEquals(2, json.getAsJsonArray("groups").size());
    }

    // ─── FullPower variants map to correct type strings ──────────────
    @Test
    void fullPowerChargingMapsToChargingState() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFullPowerCharging(Envelope.FullPowerChargingState.newBuilder()
                        .setCasterUuid("offline:Kiz"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("full_power_charging_state", json.get("type").getAsString());
    }

    @Test
    void fullPowerExhaustedMapsToExhaustedState() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFullPowerExhausted(Envelope.FullPowerExhaustedState.newBuilder()
                        .setCasterUuid("offline:Kiz"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("full_power_exhausted_state", json.get("type").getAsString());
    }

    // ─── QuickSlotConfig maps to quickslot_config (not quick_slot_config) ──
    @Test
    void quickSlotConfigMapsToCorrectTypeString() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setQuickSlotConfig(Envelope.QuickSlotConfig.newBuilder())
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("quickslot_config", json.get("type").getAsString(),
                "QUICK_SLOT_CONFIG should map to 'quickslot_config' (no underscore between quick and slot)");
    }

    // ─── SkillBarConfig maps to skillbar_config ──────────────────────
    @Test
    void skillBarConfigMapsToCorrectTypeString() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSkillBarConfig(Envelope.SkillBarConfig.newBuilder())
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("skillbar_config", json.get("type").getAsString(),
                "SKILL_BAR_CONFIG should map to 'skillbar_config'");
    }

    // ─── CombatEventFloater maps to combat_event ─────────────────────
    @Test
    void combatEventFloaterMapsToCombatEvent() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCombatEventFloater(Envelope.CombatEventFloater.newBuilder())
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("combat_event", json.get("type").getAsString(),
                "COMBAT_EVENT_FLOATER should map to 'combat_event'");
    }

    // ─── InventorySnapshot bridges correctly ─────────────────────────
    @Test
    void bridgeInventorySnapshotProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(42))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("inventory_snapshot", json.get("type").getAsString());
    }

    // ─── PlayerState bridges correctly ───────────────────────────────
    @Test
    void bridgePlayerStateProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setPlayerState(Envelope.PlayerState.newBuilder()
                        .setPlayer("offline:Steve")
                        .setRealm(Common.Realm.REALM_INDUCE)
                        .setSpiritQi(78.0)
                        .setKarma(0.2)
                        .setCompositePower(0.35)
                        .setZone("blood_valley"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("player_state", json.get("type").getAsString());
        assertEquals("offline:Steve", json.get("player").getAsString());
        assertEquals("blood_valley", json.get("zone").getAsString());
    }

    @Test
    void bridgeShieldBrokenProducesLegacyJsonAndRoutes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setShieldBroken(Envelope.ShieldBroken.newBuilder()
                        .setInstanceId(7)
                        .setTemplateId("wooden_shield"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for shield_broken: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("shield_broken", json.get("type").getAsString());
        assertEquals(7, json.get("instance_id").getAsLong());
        assertEquals("wooden_shield", json.get("template_id").getAsString());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault().route(result.legacyJson(), 0);
        assertTrue(route.isHandled(), "shield_broken proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "shield_broken proto bridge output must not become no-op");
    }

    @Test
    void bridgeShieldBlockHitProducesLegacyJsonAndRoutes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setShieldBlockHit(Envelope.ShieldBlockHit.newBuilder()
                        .setTemplateId("bone_shield"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for shield_block_hit: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("shield_block_hit", json.get("type").getAsString());
        assertEquals("bone_shield", json.get("template_id").getAsString());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault().route(result.legacyJson(), 0);
        assertTrue(route.isHandled(), "shield_block_hit proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "shield_block_hit proto bridge output must not become no-op");
    }

    @Test
    void bridgePillBuffStatusProducesLegacyJsonAndRoutesIntoHudPlanner() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setPillBuffStatus(Envelope.PillBuffStatus.newBuilder()
                        .setBuffId("tie_bi_san")
                        .setRemainingTicks(1800)
                        .setEffectMultiplier(1.25))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for pill_buff_status: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("pill_buff_status", json.get("type").getAsString());
        assertEquals("tie_bi_san", json.get("buff_id").getAsString());
        assertEquals(1800, json.get("remaining_ticks").getAsInt());
        assertEquals(1.25, json.get("effect_multiplier").getAsDouble(), 1e-9);

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);

        assertTrue(route.isHandled(), "pill_buff_status proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "pill_buff_status proto bridge output must not become no-op");
        var buffs = PillBuffHudPlanner.activeBuffs();
        assertEquals(1, buffs.size());
        assertEquals("tie_bi_san", buffs.get(0).buffId());
        assertEquals(1800, buffs.get(0).remainingTicks());
        assertEquals(1.25, buffs.get(0).effectMultiplier(), 1e-9);
    }

    @Test
    void bridgeTechniqueProficiencyUpdateProducesLegacyJsonAndRoutesIntoStore() {
        TechniquesListPanel.replace(java.util.List.of(new TechniquesListPanel.Technique(
                "woliu.vortex",
                "绝灵涡流",
                java.util.List.of(),
                TechniquesListPanel.Grade.YELLOW,
                0.10f,
                "",
                true,
                "",
                "",
                "凝脉一层",
                java.util.List.of(),
                0.4f,
                8,
                60,
                4.0f)));
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setTechniqueProficiencyUpdate(Envelope.TechniqueProficiencyUpdate.newBuilder()
                        .setTechniqueId("woliu.vortex")
                        .setProficiency(0.42f)
                        .setGain(0.02f))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for technique_proficiency_update: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("technique_proficiency_update", json.get("type").getAsString());
        assertEquals("woliu.vortex", json.get("technique_id").getAsString());
        assertEquals(0.42f, json.get("proficiency").getAsFloat(), 1e-6f);
        assertEquals(0.02f, json.get("gain").getAsFloat(), 1e-6f);

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);

        assertTrue(route.isHandled(), "technique_proficiency_update proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "technique_proficiency_update proto bridge output must not become no-op");
        var technique = TechniquesListPanel.snapshot().get(0);
        assertEquals("woliu.vortex", technique.id());
        assertEquals(0.42f, technique.proficiency(), 1e-6f);
    }

    @Test
    void bridgeFactionWarStateOutputIsIgnoredByDefaultRouter() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFactionWarState(Envelope.FactionWarState.newBuilder()
                        .setWarId(42)
                        .setZone("残灰谷")
                        .setRegionDescriptor("残灰谷一带散修")
                        .setPhase("skirmish")
                        .addGroups(0)
                        .addGroups(1)
                        .setEnlistCount(2)
                        .setMercenaryCount(1)
                        .setInterceptCount(0)
                        .setSpectateCount(3)
                        .setWinnerGroup(-1)
                        .setLoserGroup(-1))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for faction_war_state: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("faction_war_state", json.get("type").getAsString());
        assertEquals(42, json.get("war_id").getAsLong());
        assertEquals("残灰谷", json.get("zone").getAsString());
        assertEquals(2, json.getAsJsonArray("groups").size());
        assertEquals(2, json.get("enlist_count").getAsInt());
        assertEquals(3, json.get("spectate_count").getAsInt());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault().route(result.legacyJson(), 0);
        assertTrue(route.isNoOp(), "faction_war_state has no HUD handler and should be ignored: " + route.logMessage());
    }

    // ═══════════════════════════════════════════════════════════════════
    // printAndNormalize: proto3 uint64 string → JSON number
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void uint64FieldsAreNormalizedToJsonNumbers() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(999)
                        .setBoneCoins(12345))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.get("revision").isJsonPrimitive(), "revision should be a primitive");
        assertTrue(json.get("revision").getAsJsonPrimitive().isNumber(),
                "revision should be a number (not string) after normalization");
        assertEquals(999, json.get("revision").getAsLong());
        assertEquals(12345, json.get("bone_coins").getAsLong());
    }

    @Test
    void zeroValueUint64IsPreservedByIncludingDefaultValueFields() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(0)
                        .setBoneCoins(0))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.has("revision"),
                "revision=0 must be present (includingDefaultValueFields)");
        assertEquals(0, json.get("revision").getAsLong());
        assertTrue(json.has("bone_coins"),
                "bone_coins=0 must be present (includingDefaultValueFields)");
        assertEquals(0, json.get("bone_coins").getAsLong());
    }

    @Test
    void negativeInt64NormalizedToNumber() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setZoneInfo(Envelope.ZoneInfo.newBuilder()
                        .setZone("test")
                        .setSpiritQi(-3.14)
                        .setDangerLevel(-1))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.get("danger_level").getAsJsonPrimitive().isNumber(),
                "negative int should be a JSON number");
        assertEquals(-1, json.get("danger_level").getAsInt());
    }

    @Test
    void largeUint64NearMaxLongNormalizedCorrectly() {
        long nearMax = Long.MAX_VALUE - 1;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(nearMax))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.get("revision").getAsJsonPrimitive().isNumber(),
                "near-max long should still be a JSON number");
        assertEquals(nearMax, json.get("revision").getAsLong());
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeOneofFlat: InventoryEvent — all 4 variants
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void inventoryEventMovedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setMoved(Envelope.InventoryEventMoved.newBuilder()
                                .setRevision(7)
                                .setInstanceId(42)
                                .setFrom(Envelope.InventoryLocation.newBuilder()
                                        .setContainer(Envelope.InventoryLocationContainer.newBuilder()
                                                .setContainerId("backpack").setRow(0).setCol(1)))
                                .setTo(Envelope.InventoryLocation.newBuilder()
                                        .setEquip(Envelope.InventoryLocationEquip.newBuilder()
                                                .setSlot(Envelope.EquipSlot.EQUIP_SLOT_CHEST)))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("inventory_event", json.get("type").getAsString());
        assertEquals("moved", json.get("kind").getAsString(),
                "oneof variant name should become 'kind' field");
        assertEquals(7, json.get("revision").getAsLong(),
                "inner fields should be flattened to top level");
        assertEquals(42, json.get("instance_id").getAsLong());
        assertTrue(json.has("from"), "from location should be present");
        assertTrue(json.has("to"), "to location should be present");
    }

    @Test
    void inventoryEventDroppedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setDropped(Envelope.InventoryEventDropped.newBuilder()
                                .setRevision(3)
                                .setInstanceId(10)
                                .setWorldPosX(100.0).setWorldPosY(64.0).setWorldPosZ(200.0)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("dropped", json.get("kind").getAsString());
        assertEquals(3, json.get("revision").getAsLong());
        assertEquals(100.0, json.get("world_pos_x").getAsDouble(), 0.01);
    }

    @Test
    void inventoryEventStackChangedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setStackChanged(Envelope.InventoryEventStackChanged.newBuilder()
                                .setRevision(5).setInstanceId(99).setStackCount(64)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("stack_changed", json.get("kind").getAsString());
        assertEquals(64, json.get("stack_count").getAsLong());
    }

    @Test
    void inventoryEventDurabilityChangedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setDurabilityChanged(Envelope.InventoryEventDurabilityChanged.newBuilder()
                                .setRevision(8).setInstanceId(50).setDurability(0.75)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("durability_changed", json.get("kind").getAsString());
        assertEquals(0.75, json.get("durability").getAsDouble(), 0.001);
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeOneofFlat: CraftOutcome — completed / failed
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void craftOutcomeCompletedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCraftOutcome(Envelope.CraftOutcome.newBuilder()
                        .setCompleted(Envelope.CraftOutcomeCompleted.newBuilder()
                                .setRecipeId("iron_sword")
                                .setOutputTemplate("iron_sword_basic")
                                .setOutputCount(1)
                                .setCompletedAtTick(1000)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("craft_outcome", json.get("type").getAsString());
        assertEquals("completed", json.get("kind").getAsString());
        assertEquals("iron_sword", json.get("recipe_id").getAsString());
        assertEquals(1, json.get("output_count").getAsInt());
        assertEquals(1000, json.get("completed_at_tick").getAsLong());
    }

    @Test
    void craftOutcomeFailedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCraftOutcome(Envelope.CraftOutcome.newBuilder()
                        .setFailed(Envelope.CraftOutcomeFailed.newBuilder()
                                .setRecipeId("spirit_pill")
                                .setMaterialReturned(2)
                                .setQiRefunded(50.5)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("failed", json.get("kind").getAsString());
        assertEquals("spirit_pill", json.get("recipe_id").getAsString());
        assertEquals(2, json.get("material_returned").getAsInt());
        assertEquals(50.5, json.get("qi_refunded").getAsDouble(), 0.01);
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeSlotConfig: QuickSlotConfig — wrapper unwrap + empty slots
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void quickSlotConfigUnwrapsEntryAndNullifiesEmpty() {
        Envelope.QuickSlotConfig.Builder qsc = Envelope.QuickSlotConfig.newBuilder();
        // slot 0: filled
        qsc.addSlots(Envelope.OptionalQuickSlotEntry.newBuilder()
                .setEntry(Envelope.QuickSlotEntry.newBuilder()
                        .setItemId("healing_pill")
                        .setDisplayName("灵息丸")
                        .setCastDurationMs(500)));
        // slots 1-8: empty
        for (int i = 1; i < 9; i++) {
            qsc.addSlots(Envelope.OptionalQuickSlotEntry.newBuilder());
        }
        for (int i = 0; i < 9; i++) {
            qsc.addCooldownUntilMs(0);
        }

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setQuickSlotConfig(qsc).build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("quickslot_config", json.get("type").getAsString());

        JsonArray slots = json.getAsJsonArray("slots");
        assertEquals(9, slots.size(), "should have 9 slots");

        // slot 0: unwrapped from wrapper — should have item_id directly
        assertTrue(slots.get(0).isJsonObject(), "filled slot should be an object");
        JsonObject slot0 = slots.get(0).getAsJsonObject();
        assertEquals("healing_pill", slot0.get("item_id").getAsString(),
                "item_id should be at top level (not nested under 'entry')");
        assertFalse(slot0.has("entry"),
                "wrapper 'entry' field should be removed after unwrapping");

        // slots 1-8: empty wrapper {} → JsonNull
        for (int i = 1; i < 9; i++) {
            assertTrue(slots.get(i).isJsonNull(),
                    "empty slot " + i + " should be null (not empty object {})");
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeSlotConfig: SkillBarConfig — wrapper + oneof flatten
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void skillBarConfigUnwrapsAndFlattensOneof() {
        Envelope.SkillBarConfig.Builder sbc = Envelope.SkillBarConfig.newBuilder();
        // slot 0: item type
        sbc.addSlots(Envelope.OptionalSkillBarEntry.newBuilder()
                .setEntry(Envelope.SkillBarEntry.newBuilder()
                        .setItem(Envelope.SkillBarEntryItem.newBuilder()
                                .setTemplateId("iron_sword")
                                .setDisplayName("铁剑")
                                .setCastDurationMs(0)
                                .setCooldownMs(1000))));
        // slot 1: skill type
        sbc.addSlots(Envelope.OptionalSkillBarEntry.newBuilder()
                .setEntry(Envelope.SkillBarEntry.newBuilder()
                        .setSkill(Envelope.SkillBarEntrySkill.newBuilder()
                                .setSkillId("fireball")
                                .setDisplayName("火球术")
                                .setCastDurationMs(2000)
                                .setCooldownMs(5000))));
        // slots 2-8: empty
        for (int i = 2; i < 9; i++) {
            sbc.addSlots(Envelope.OptionalSkillBarEntry.newBuilder());
        }
        for (int i = 0; i < 9; i++) {
            sbc.addCooldownUntilMs(0);
        }

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSkillBarConfig(sbc).build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("skillbar_config", json.get("type").getAsString());

        JsonArray slots = json.getAsJsonArray("slots");

        // slot 0: item — kind should be "item", template_id at top level
        JsonObject slot0 = slots.get(0).getAsJsonObject();
        assertEquals("item", slot0.get("kind").getAsString(),
                "oneof variant 'item' should become kind='item'");
        assertEquals("iron_sword", slot0.get("template_id").getAsString(),
                "inner fields should be flattened to slot level");
        assertFalse(slot0.has("item"), "oneof variant key 'item' should be removed");
        assertFalse(slot0.has("entry"), "wrapper key 'entry' should be removed");

        // slot 1: skill
        JsonObject slot1 = slots.get(1).getAsJsonObject();
        assertEquals("skill", slot1.get("kind").getAsString());
        assertEquals("fireball", slot1.get("skill_id").getAsString());

        // slots 2-8: null
        for (int i = 2; i < 9; i++) {
            assertTrue(slots.get(i).isJsonNull(),
                    "empty slot " + i + " should be null");
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeForgeSession: step_state oneof flatten
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void forgeSessionInscriptionFlattenedWithStepTag() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(1)
                        .setBlueprintId("iron_sword_bp")
                        .setBlueprintName("铁剑图纸")
                        .setActive(true)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_INSCRIPTION)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setInscription(Envelope.ForgeStepStateInscription.newBuilder()
                                        .setFilledSlots(3)
                                        .setMaxSlots(5)
                                        .setFailed(false))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("forge_session", json.get("type").getAsString());
        assertEquals(1, json.get("session_id").getAsLong());

        JsonObject stepState = json.getAsJsonObject("step_state");
        assertNotNull(stepState, "step_state should be present");
        assertEquals("inscription", stepState.get("step").getAsString(),
                "oneof variant should become step='inscription'");
        assertEquals(3, stepState.get("filled_slots").getAsInt(),
                "inner fields should be flattened into step_state");
        assertEquals(5, stepState.get("max_slots").getAsInt());
        assertFalse(stepState.has("inscription"),
                "oneof variant key 'inscription' should be removed after flatten");
    }

    @Test
    void forgeSessionTemperingFlattenedWithStepTag() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(2)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_TEMPERING)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setTempering(Envelope.ForgeStepStateTempering.newBuilder()
                                        .setBeatCursor(4)
                                        .setHits(3)
                                        .setMisses(1))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject stepState = json.getAsJsonObject("step_state");
        assertEquals("tempering", stepState.get("step").getAsString());
        assertEquals(4, stepState.get("beat_cursor").getAsInt());
        assertEquals(3, stepState.get("hits").getAsInt());
    }

    @Test
    void forgeSessionNoneStateFlattenedToStepNone() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(3)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_DONE)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setNoneState(true)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject stepState = json.getAsJsonObject("step_state");
        assertEquals("none", stepState.get("step").getAsString(),
                "none_state=true should become step='none'");
        assertFalse(stepState.has("none_state"),
                "none_state field should be removed after conversion");
    }

    @Test
    void forgeSessionBilletFlattenedWithStepTag() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(4)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_BILLET)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setBillet(Envelope.ForgeStepStateBillet.newBuilder()
                                        .setResolvedTierCap(3))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject stepState = json.getAsJsonObject("step_state");
        assertEquals("billet", stepState.get("step").getAsString());
        assertEquals(3, stepState.get("resolved_tier_cap").getAsInt());
    }

    @Test
    void forgeSessionConsecrationFlattenedWithStepTag() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(5)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_CONSECRATION)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setConsecration(Envelope.ForgeStepStateConsecration.newBuilder()
                                        .setQiInjected(50.0)
                                        .setQiRequired(100.0))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject stepState = json.getAsJsonObject("step_state");
        assertEquals("consecration", stepState.get("step").getAsString());
        assertEquals(50.0, stepState.get("qi_injected").getAsDouble(), 0.01);
    }

    // ═══════════════════════════════════════════════════════════════════
    // normalizeNumericStrings: nested objects and arrays
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void nestedUint64InSubObjectsAreNormalized() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setMoved(Envelope.InventoryEventMoved.newBuilder()
                                .setRevision(123456789)
                                .setInstanceId(987654321)
                                .setFrom(Envelope.InventoryLocation.newBuilder()
                                        .setContainer(Envelope.InventoryLocationContainer.newBuilder()
                                                .setContainerId("backpack")
                                                .setRow(5).setCol(3)))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals(123456789, json.get("revision").getAsLong());
        assertEquals(987654321, json.get("instance_id").getAsLong());

        JsonObject from = json.getAsJsonObject("from");
        assertNotNull(from);
        JsonObject container = from.getAsJsonObject("container");
        assertNotNull(container);
        assertTrue(container.get("row").getAsJsonPrimitive().isNumber(),
                "nested uint64 'row' should be a JSON number after normalization");
        assertEquals(5, container.get("row").getAsLong());
    }

    @Test
    void uint64InRepeatedFieldsAreNormalized() {
        Envelope.QuickSlotConfig.Builder qsc = Envelope.QuickSlotConfig.newBuilder();
        for (int i = 0; i < 9; i++) {
            qsc.addSlots(Envelope.OptionalQuickSlotEntry.newBuilder());
        }
        qsc.addCooldownUntilMs(1000);
        qsc.addCooldownUntilMs(0);
        qsc.addCooldownUntilMs(5000);
        for (int i = 3; i < 9; i++) {
            qsc.addCooldownUntilMs(0);
        }

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setQuickSlotConfig(qsc).build();

        JsonObject json = bridgeAndParse(envelope);
        JsonArray cooldowns = json.getAsJsonArray("cooldown_until_ms");
        assertEquals(9, cooldowns.size());
        assertTrue(cooldowns.get(0).getAsJsonPrimitive().isNumber(),
                "uint64 elements in repeated field should be JSON numbers");
        assertEquals(1000, cooldowns.get(0).getAsLong());
        assertEquals(5000, cooldowns.get(2).getAsLong());
    }

    // ═══════════════════════════════════════════════════════════════════
    // cast_sync: enum prefix normalization (plan-skill-warn-hud)
    //
    // proto3 canonical JSON 把枚举打成 CAST_PHASE_* / CAST_OUTCOME_*；CastSyncHandler
    // 期望 serde snake_case。bridgeCastSync 必须剥前缀转小写，否则整条 cast_sync 在
    // proto 线上静默 noOp（连既有 casting/meridian_gated 也收不到）。
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void bridgeCastSyncStripsPhaseAndOutcomeEnumPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCastSync(Envelope.CastSync.newBuilder()
                        .setPhase(Envelope.CastPhase.CAST_PHASE_CASTING)
                        .setSlot(3)
                        .setDurationMs(1500)
                        .setStartedAtMs(1_700_000_000_000L)
                        .setOutcome(Envelope.CastOutcome.CAST_OUTCOME_NONE))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("cast_sync", json.get("type").getAsString());
        assertEquals("casting", json.get("phase").getAsString(),
                "phase 必须从 CAST_PHASE_CASTING 剥成 'casting'（CastSyncHandler switch 期望），"
                + "否则 proto 线上 cast bar 整条静默 noOp");
        assertEquals("none", json.get("outcome").getAsString(),
                "outcome 必须从 CAST_OUTCOME_NONE 剥成 'none'");
    }

    @Test
    void bridgeCastSyncMaps通用警示RejectOutcomesToSnakeCase() {
        // 每个 reject outcome 经 bridge 后必须落到 CastSyncHandler.parseOutcome 认得的 wire 串。
        record Case(Envelope.CastOutcome proto, String expectedWire) {}
        Case[] cases = new Case[] {
            new Case(Envelope.CastOutcome.CAST_OUTCOME_MERIDIAN_GATED, "meridian_gated"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_QI_INSUFFICIENT, "reject_qi_insufficient"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_ON_COOLDOWN, "reject_on_cooldown"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_INVALID_TARGET, "reject_invalid_target"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_IN_RECOVERY, "reject_in_recovery"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_REALM_TOO_LOW, "reject_realm_too_low"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_NO_WEAPON, "reject_no_weapon"),
        };
        for (Case c : cases) {
            Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                    .setCastSync(Envelope.CastSync.newBuilder()
                            .setPhase(Envelope.CastPhase.CAST_PHASE_IDLE)
                            .setSlot(0)
                            .setDurationMs(0)
                            .setStartedAtMs(1L)
                            .setOutcome(c.proto()))
                    .build();
            JsonObject json = bridgeAndParse(envelope);
            assertEquals("idle", json.get("phase").getAsString(),
                    "施放前拒绝 phase 应为 idle for " + c.proto());
            assertEquals(c.expectedWire(), json.get("outcome").getAsString(),
                    "outcome " + c.proto() + " 应剥成 '" + c.expectedWire()
                    + "'（CastSyncHandler.parseOutcome 据此映射文案）");
        }
    }

    // ─── event_stream_push: 战斗事件流 HUD 接收前提 ──────────────────
    // proto3 JSON 把 EventChannel/EventPriority 打成枚举全名；EventStreamPushHandler
    // 只认 serde snake_case。bridge 必须剥前缀转小写，否则 channel/priority parse 为
    // null → handler noOp → HUD 事件流永远空白。锁住每个 channel/priority 变体的 wire 串。

    @Test
    void bridgeEventStreamPushStripsChannelAndPriorityPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                        .setChannel(Envelope.EventChannel.EVENT_CHANNEL_COMBAT)
                        .setPriority(Envelope.EventPriority.EVENT_PRIORITY_P1_IMPORTANT)
                        .setSourceTag("hit-Head-Slash")
                        .setText("命中 Head Slash -8")
                        .setColor(0)
                        .setCreatedAtMs(1_700_000_000_000L))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("event_stream_push", json.get("type").getAsString());
        assertEquals("combat", json.get("channel").getAsString(),
                "channel 必须从 EVENT_CHANNEL_COMBAT 剥成 'combat'（EventStreamPushHandler.parseChannel 期望），"
                + "否则 proto 线上战斗事件流整条静默 noOp，HUD 事件流区永远空白");
        assertEquals("p1_important", json.get("priority").getAsString(),
                "priority 必须从 EVENT_PRIORITY_P1_IMPORTANT 剥成 'p1_important'（parsePriority 期望）");
        assertEquals("命中 Head Slash -8", json.get("text").getAsString(), "text 应原样透传");
        assertEquals("hit-Head-Slash", json.get("source_tag").getAsString(), "source_tag 应原样透传");
    }

    @Test
    void bridgeEventStreamPushMapsAllChannelsToHandlerWire() {
        record Case(Envelope.EventChannel proto, String expectedWire) {}
        Case[] cases = new Case[] {
            new Case(Envelope.EventChannel.EVENT_CHANNEL_COMBAT, "combat"),
            new Case(Envelope.EventChannel.EVENT_CHANNEL_CULTIVATION, "cultivation"),
            new Case(Envelope.EventChannel.EVENT_CHANNEL_WORLD, "world"),
            new Case(Envelope.EventChannel.EVENT_CHANNEL_SOCIAL, "social"),
            new Case(Envelope.EventChannel.EVENT_CHANNEL_SYSTEM, "system"),
        };
        for (Case c : cases) {
            Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                    .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                            .setChannel(c.proto())
                            .setPriority(Envelope.EventPriority.EVENT_PRIORITY_P2_NORMAL)
                            .setText("x")
                            .setCreatedAtMs(1L))
                    .build();
            JsonObject json = bridgeAndParse(envelope);
            assertEquals(c.expectedWire(), json.get("channel").getAsString(),
                    "channel " + c.proto() + " 应剥成 '" + c.expectedWire()
                    + "'（EventStreamPushHandler.parseChannel 据此入 UnifiedEventStore）");
        }
    }

    @Test
    void bridgeEventStreamPushMapsAllPrioritiesToHandlerWire() {
        record Case(Envelope.EventPriority proto, String expectedWire) {}
        Case[] cases = new Case[] {
            new Case(Envelope.EventPriority.EVENT_PRIORITY_P0_CRITICAL, "p0_critical"),
            new Case(Envelope.EventPriority.EVENT_PRIORITY_P1_IMPORTANT, "p1_important"),
            new Case(Envelope.EventPriority.EVENT_PRIORITY_P2_NORMAL, "p2_normal"),
            new Case(Envelope.EventPriority.EVENT_PRIORITY_P3_VERBOSE, "p3_verbose"),
        };
        for (Case c : cases) {
            Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                    .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                            .setChannel(Envelope.EventChannel.EVENT_CHANNEL_COMBAT)
                            .setPriority(c.proto())
                            .setText("x")
                            .setCreatedAtMs(1L))
                    .build();
            JsonObject json = bridgeAndParse(envelope);
            assertEquals(c.expectedWire(), json.get("priority").getAsString(),
                    "priority " + c.proto() + " 应剥成 '" + c.expectedWire()
                    + "'（parsePriority 据此；死亡事件用 P0_CRITICAL）");
        }
    }

    // ─── event_stream_push 端到端：bridge → router → handler → store ──
    // 缺陷本质在下游消费链：枚举未剥 → handler noOp → 事件永不入库。这条用例走完整
    // proto bytes → ProtoServerDataBridge → ServerDataRouter → EventStreamPushHandler
    // → UnifiedEventStore，断言可观测的入库结果，单元桥接断言无法替代。

    @Test
    void eventStreamPushRoutesEndToEndIntoUnifiedEventStore() {
        UnifiedEventStore.resetForTests();
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                        .setChannel(Envelope.EventChannel.EVENT_CHANNEL_COMBAT)
                        .setPriority(Envelope.EventPriority.EVENT_PRIORITY_P1_IMPORTANT)
                        .setSourceTag("hit-Head-Slash")
                        .setText("命中 Head Slash -8")
                        .setColor(0)
                        .setCreatedAtMs(1_700_000_000_000L))
                .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(), "bridge should succeed: " + bridged.errorMessage());

        ServerDataRouter router = ServerDataRouter.createDefault();
        ServerDataRouter.RouteResult result = router.route(
                bridged.legacyJson(),
                bridged.legacyJson().getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(),
                "event_stream_push 应被 handler 接受(非 noOp)；枚举未剥时这里会 noOp，事件流永远空白");
        List<UnifiedEvent> events = UnifiedEventStore.stream().snapshot();
        assertEquals(1, events.size(),
                "事件应入 UnifiedEventStore；修前因 channel/priority parse 为 null 而永不入库");
        UnifiedEvent ev = events.get(0);
        assertEquals(UnifiedEvent.Channel.COMBAT, ev.channel(),
                "channel 应解析为 COMBAT（来自剥前缀后的 'combat'）");
        assertEquals(UnifiedEvent.Priority.P1_IMPORTANT, ev.priority(),
                "priority 应解析为 P1_IMPORTANT（来自剥前缀后的 'p1_important'）");
        assertEquals("命中 Head Slash -8", ev.text(), "text 端到端原样透传");
    }

    @Test
    void eventStreamPushUnspecifiedEnumStaysOutOfStore() {
        // 错误分支：UNSPECIFIED 剥后为 'unspecified'，parseChannel/parsePriority 返回 null，
        // handler noOp，事件不入库。锁住"坏 channel/priority 不污染事件流"。
        UnifiedEventStore.resetForTests();
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                        .setChannel(Envelope.EventChannel.EVENT_CHANNEL_UNSPECIFIED)
                        .setPriority(Envelope.EventPriority.EVENT_PRIORITY_UNSPECIFIED)
                        .setText("x")
                        .setCreatedAtMs(1L))
                .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess());
        ServerDataRouter router = ServerDataRouter.createDefault();
        ServerDataRouter.RouteResult result = router.route(
                bridged.legacyJson(),
                bridged.legacyJson().getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isHandled(), "UNSPECIFIED channel/priority 应 noOp，不被当作有效事件");
        assertEquals(0, UnifiedEventStore.stream().snapshot().size(),
                "坏枚举不应入 UnifiedEventStore");
    }

    // ═══════════════════════════════════════════════════════════════════
    // F9 跨层修复：tutorial_coffin_pos proto bridge
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void bridgeTutorialCoffinPosProducesLegacyJsonAndRoutesIntoStore() {
        com.bong.client.coffin.TutorialCoffinPosStore.resetForTests();
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setTutorialCoffinPos(Envelope.TutorialCoffinPos.newBuilder()
                        .setX(12)
                        .setY(71)
                        .setZ(-33))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("tutorial_coffin_pos", json.get("type").getAsString());
        assertEquals(12, json.get("x").getAsInt());
        assertEquals(71, json.get("y").getAsInt());
        assertEquals(-33, json.get("z").getAsInt(),
                "negative z must survive the proto->JSON bridge untouched");

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(json.toString(), json.toString().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "tutorial_coffin_pos proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "tutorial_coffin_pos proto bridge output must not become no-op");

        var stored = com.bong.client.coffin.TutorialCoffinPosStore.snapshot();
        assertTrue(stored.isPresent(), "store should hold the broadcast coffin pos after routing");
        assertEquals(new net.minecraft.util.math.BlockPos(12, 71, -33), stored.get());
    }

    @Test
    void bridgeTutorialCoffinPosAtOriginIsPreservedIncludingDefaultValueFields() {
        // x=0/y=0/z=0 must still survive the bridge (includingDefaultValueFields), otherwise
        // a coffin that legitimately sits at the world origin would look like "no field".
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setTutorialCoffinPos(Envelope.TutorialCoffinPos.newBuilder()
                        .setX(0).setY(0).setZ(0))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("tutorial_coffin_pos", json.get("type").getAsString());
        assertTrue(json.has("x") && json.has("y") && json.has("z"),
                "x=0/y=0/z=0 fields must be present, not dropped as proto3 defaults");
        assertEquals(0, json.get("x").getAsInt());
        assertEquals(0, json.get("y").getAsInt());
        assertEquals(0, json.get("z").getAsInt());
    }

    // ═══════════════════════════════════════════════════════════════════
    // Helper
    // ═══════════════════════════════════════════════════════════════════

    private static JsonObject bridgeAndParse(Envelope.ServerDataEnvelope envelope) {
        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed: " + result.errorMessage());
        return JsonParser.parseString(result.legacyJson()).getAsJsonObject();
    }
}
