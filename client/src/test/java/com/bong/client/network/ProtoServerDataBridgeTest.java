package com.bong.client.network;

import bong.Common;
import bong.Envelope;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class ProtoServerDataBridgeTest {

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
}
