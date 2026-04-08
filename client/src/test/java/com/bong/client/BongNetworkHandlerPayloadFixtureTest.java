package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

public class BongNetworkHandlerPayloadFixtureTest {
    private static final Path SCHEMA_SAMPLES_DIR = Path.of("..", "agent", "packages", "schema", "samples");

    @BeforeEach
    void setUp() {
        NarrationState.clear();
        ZoneState.clear();
        EventAlertState.clear();
        PlayerStateState.clear();
    }

    @AfterEach
    void tearDown() {
        NarrationState.clear();
        ZoneState.clear();
        EventAlertState.clear();
        PlayerStateState.clear();
    }

    private static String loadFixture(String fileName) {
        Path fixturePath = SCHEMA_SAMPLES_DIR.resolve(fileName);
        assertTrue(Files.exists(fixturePath), "Fixture should exist: " + fixturePath.toAbsolutePath());

        try {
            return Files.readString(fixturePath, StandardCharsets.UTF_8);
        } catch (IOException e) {
            fail("Failed to read fixture: " + fixturePath.toAbsolutePath() + " error=" + e.getMessage());
            return null;
        }
    }

    @Test
    public void sharedWelcomeFixtureParsesSuccessfully() {
        String json = loadFixture("client-payload-welcome.sample.json");
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "welcome fixture should parse successfully");
        BongServerPayload.WelcomePayload payload = assertInstanceOf(BongServerPayload.WelcomePayload.class, result.payload);
        assertEquals(1, payload.v());
        assertEquals("welcome", payload.type());
        assertEquals("欢迎踏入洞天，天道正在观测你的命数。", payload.message());
        assertTrue(BongServerPayloadRouter.route(null, payload));
    }

    @Test
    public void sharedHeartbeatFixtureParsesSuccessfully() {
        String json = loadFixture("client-payload-heartbeat.sample.json");
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "heartbeat fixture should parse successfully");
        BongServerPayload.HeartbeatPayload payload = assertInstanceOf(BongServerPayload.HeartbeatPayload.class, result.payload);
        assertEquals(1, payload.v());
        assertEquals("heartbeat", payload.type());
        assertEquals("server tick 84000", payload.message());
        assertTrue(BongServerPayloadRouter.route(null, payload));
    }

    @Test
    public void sharedNarrationFixtureParsesSuccessfully() {
        String json = loadFixture("client-payload-narration.sample.json");
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "narration fixture should parse successfully");
        BongServerPayload.NarrationPayload payload = assertInstanceOf(BongServerPayload.NarrationPayload.class, result.payload);
        assertEquals(1, payload.v());
        assertEquals("narration", payload.type());
        assertEquals(1, payload.narrations().size());
        assertEquals("broadcast", payload.narrations().get(0).scope());
        assertEquals("天道震怒，血谷上空乌云密布，一道紫雷即将降下……", payload.narrations().get(0).text());
        assertEquals("system_warning", payload.narrations().get(0).style());
        assertTrue(BongServerPayloadRouter.route(null, payload));
    }

    @Test
    public void sharedZoneInfoFixtureParsesSuccessfully() {
        String json = loadFixture("client-payload-zone-info.sample.json");
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "zone_info fixture should parse successfully");
        BongServerPayload.ZoneInfoPayload payload = assertInstanceOf(BongServerPayload.ZoneInfoPayload.class, result.payload);
        assertEquals(1, payload.v());
        assertEquals("zone_info", payload.type());
        assertEquals("blood_valley", payload.zoneInfo().zone());
        assertEquals(0.42d, payload.zoneInfo().spiritQi());
        assertEquals(3, payload.zoneInfo().dangerLevel());
        assertEquals(1, payload.zoneInfo().activeEvents().size());
        assertEquals("beast_tide_warning", payload.zoneInfo().activeEvents().get(0));
        assertTrue(BongServerPayloadRouter.route(null, payload));
        assertEquals("Blood Valley", ZoneState.getCurrentZone().zoneLabel());
        assertEquals(0.42d, ZoneState.getCurrentZone().spiritQi());
        assertEquals(3, ZoneState.getCurrentZone().dangerLevel());
    }

    @Test
    public void sharedEventAlertFixtureParsesSuccessfully() {
        String json = loadFixture("client-payload-event-alert.sample.json");
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "event_alert fixture should parse successfully");
        BongServerPayload.EventAlertPayload payload = assertInstanceOf(BongServerPayload.EventAlertPayload.class, result.payload);
        assertEquals(1, payload.v());
        assertEquals("event_alert", payload.type());
        assertEquals("thunder_tribulation", payload.eventAlert().kind());
        assertEquals("雷劫将至", payload.eventAlert().title());
        assertEquals("血谷上空劫云汇聚，三十息内可能落雷。", payload.eventAlert().detail());
        assertEquals("critical", payload.eventAlert().severity());
        assertEquals("blood_valley", payload.eventAlert().zone());
        assertTrue(BongServerPayloadRouter.route(null, payload));
        assertEquals("雷劫将至", EventAlertState.getCurrentBanner(1_000L).title());
        assertEquals(EventAlertState.Severity.CRITICAL, EventAlertState.getCurrentBanner(1_000L).severity());
        assertEquals("Blood Valley", EventAlertState.getCurrentBanner(1_000L).zoneLabel());
    }

    @Test
    public void sharedPlayerStateFixtureParsesSuccessfully() {
        String json = loadFixture("client-payload-player-state.sample.json");
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "player_state fixture should parse successfully");
        BongServerPayload.PlayerStatePayload payload = assertInstanceOf(BongServerPayload.PlayerStatePayload.class, result.payload);
        assertEquals(1, payload.v());
        assertEquals("player_state", payload.type());
        assertEquals("qi_refining_3", payload.playerState().realm());
        assertEquals(78d, payload.playerState().spiritQi());
        assertEquals(100d, payload.playerState().spiritQiMax());
        assertEquals(-0.2d, payload.playerState().karma());
        assertEquals(0.35d, payload.playerState().compositePower());
        assertEquals("blood_valley", payload.playerState().zone());
        assertTrue(BongServerPayloadRouter.route(null, payload));

        PlayerStateState.PlayerStateSnapshot snapshot = PlayerStateState.getCurrentPlayerState();
        assertNotNull(snapshot);
        assertEquals("qi_refining_3", snapshot.realmKey());
        assertEquals("blood_valley", snapshot.zoneKey());
    }

    @Test
    public void malformedJsonIsRejectedWithinPayloadFixtureSuite() {
        String malformed = loadFixture("client-payload-welcome.sample.json")
                .replace(",\n  \"message\"", "\n  \"message\"");

        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(malformed);

        assertFalse(result.success, "malformed payload JSON should fail parsing");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Malformed JSON"));
    }

    @Test
    public void unsupportedVersionIsRejectedWithinPayloadFixtureSuite() {
        String unsupportedVersion = loadFixture("client-payload-heartbeat.sample.json")
                .replace("\"v\": 1", "\"v\": 2");

        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(unsupportedVersion);

        assertFalse(result.success, "unsupported version should fail parsing");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Unsupported version"));
    }

    @Test
    public void unknownTypeIsRejectedWithinPayloadFixtureSuite() {
        String unknownType = loadFixture("client-payload-welcome.sample.json")
                .replace("\"type\": \"welcome\"", "\"type\": \"unknown_payload\"");

        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(unknownType);

        assertFalse(result.success, "unknown type should fail parsing");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Unknown payload type"));
    }
}
