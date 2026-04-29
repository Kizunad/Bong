package com.bong.client.network;

import com.bong.client.state.ZoneState;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ZoneInfoHandlerTest {
    @Test
    void parsesZonePayloadAndClampsDisplayValues() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-zone-info.json");
        ServerDataDispatch dispatch = new ZoneInfoHandler(() -> 4_242L).handle(parseEnvelope(json));

        assertTrue(dispatch.handled());
        ZoneState zoneState = dispatch.zoneState().orElseThrow();
        assertEquals("blood_valley", zoneState.zoneId());
        assertEquals("Blood Valley", zoneState.zoneLabel());
        assertEquals(1.0, zoneState.spiritQiNormalized(), 0.0001);
        assertEquals(5, zoneState.dangerLevel());
        assertEquals("collapsed", zoneState.status());
        assertEquals(4_242L, zoneState.changedAtMillis());
    }

    @Test
    void fallsBackToZoneIdWhenDisplayNameMissing() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-zone-info-no-display-name.json");
        ServerDataDispatch dispatch = new ZoneInfoHandler(() -> 88L).handle(parseEnvelope(json));

        assertTrue(dispatch.handled());
        ZoneState zoneState = dispatch.zoneState().orElseThrow();
        assertEquals("jade_valley", zoneState.zoneLabel());
        assertEquals("normal", zoneState.status());
    }

    @Test
    void invalidRequiredFieldsBecomeSafeNoOp() {
        ServerDataDispatch dispatch = new ZoneInfoHandler(() -> 1L).handle(parseEnvelope(
            "{\"v\":1,\"type\":\"zone_info\",\"zone\":\"blood_valley\",\"danger_level\":3}"
        ));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.zoneState().isEmpty());
        assertTrue(dispatch.logMessage().contains("required fields"));
    }

    @Test
    void fractionalDangerLevelBecomesSafeNoOp() {
        ServerDataDispatch dispatch = new ZoneInfoHandler(() -> 1L).handle(parseEnvelope(
            "{\"v\":1,\"type\":\"zone_info\",\"zone\":\"blood_valley\",\"spirit_qi\":0.75,\"danger_level\":3.9}"
        ));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.zoneState().isEmpty());
        assertTrue(dispatch.logMessage().contains("required fields"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), () -> "Expected payload to parse successfully but got: " + parseResult.errorMessage());
        return parseResult.envelope();
    }
}
