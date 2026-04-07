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
    void defaultRouterRegistersExactlySevenTypes() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        assertEquals(Set.of(
            "welcome",
            "heartbeat",
            "narration",
            "zone_info",
            "event_alert",
            "player_state",
            "ui_open"
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
