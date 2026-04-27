package com.bong.client.network;

import com.bong.client.state.PlayerStateViewModel;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class PlayerStateHandlerTest {
    private final PlayerStateHandler handler = new PlayerStateHandler();

    @Test
    void mapsFixturePayloadIntoViewModel() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-player-state.json");

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));
        PlayerStateViewModel playerState = dispatch.playerStateViewModel().orElseThrow();

        assertTrue(dispatch.handled());
        assertTrue(dispatch.chatMessages().isEmpty());
        assertEquals("Induce", playerState.realm());
        assertEquals(78.0, playerState.spiritQiCurrent(), 0.0001);
        assertEquals(100.0, playerState.spiritQiMax(), 0.0001);
        assertEquals(0.78, playerState.spiritQiFillRatio(), 0.0001);
        assertEquals(0.20, playerState.karma(), 0.0001);
        assertEquals(0.35, playerState.compositePower(), 0.0001);
        assertEquals(0.20, playerState.breakdown().combat(), 0.0001);
        assertEquals(0.40, playerState.breakdown().wealth(), 0.0001);
        assertEquals(0.65, playerState.breakdown().social(), 0.0001);
        assertEquals(0.10, playerState.breakdown().territory(), 0.0001);
        assertEquals("green_cloud_peak", playerState.zoneId());
        assertEquals("青云峰", playerState.zoneLabel());
        assertEquals(0.78, playerState.zoneSpiritQiNormalized(), 0.0001);
    }

    @Test
    void acceptsLegacySpiritQiAliasAndViewModelClampRules() {
        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"player_state","realm":" Condense ","spirit_qi":150.0,
             "karma":2.0,"composite_power":-1.0,
              "breakdown":{"combat":1.4,"wealth":-0.5,"social":0.2,"territory":2.0},
              "zone":" azure_peak ","zone_spirit_qi":1.6}
            """));

        PlayerStateViewModel playerState = dispatch.playerStateViewModel().orElseThrow();

        assertTrue(dispatch.handled());
        assertEquals("Condense", playerState.realm());
        assertEquals(150.0, playerState.spiritQiMax(), 0.0001);
        assertEquals(150.0, playerState.spiritQiCurrent(), 0.0001);
        assertEquals(1.0, playerState.karma(), 0.0001);
        assertEquals(0.0, playerState.compositePower(), 0.0001);
        assertEquals(1.0, playerState.breakdown().combat(), 0.0001);
        assertEquals(0.0, playerState.breakdown().wealth(), 0.0001);
        assertEquals(0.2, playerState.breakdown().social(), 0.0001);
        assertEquals(1.0, playerState.breakdown().territory(), 0.0001);
        assertEquals("azure_peak", playerState.zoneLabel());
        assertEquals(1.0, playerState.zoneSpiritQiNormalized(), 0.0001);
    }

    @Test
    void acceptsServerCompatiblePayloadWhenZoneSpiritQiIsOmitted() {
        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"player_state","realm":"Induce","spirit_qi":78.0,
             "karma":0.2,"composite_power":0.35,
              "breakdown":{"combat":0.2,"wealth":0.4,"social":0.65,"territory":0.1},
              "zone":"blood_valley"}
            """));

        PlayerStateViewModel playerState = dispatch.playerStateViewModel().orElseThrow();

        assertTrue(dispatch.handled());
        assertEquals("Induce", playerState.realm());
        assertEquals(78.0, playerState.spiritQiCurrent(), 0.0001);
        assertEquals(100.0, playerState.spiritQiMax(), 0.0001);
        assertEquals(0.78, playerState.spiritQiFillRatio(), 0.0001);
        assertEquals("blood_valley", playerState.zoneId());
        assertEquals("blood_valley", playerState.zoneLabel());
        assertEquals(0.0, playerState.zoneSpiritQiNormalized(), 0.0001);
    }

    @Test
    void missingRequiredFieldsReturnSafeNoOp() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-player-state-missing-fields.json");

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.playerStateViewModel().isEmpty());
        assertTrue(dispatch.logMessage().contains("realm"));
        assertTrue(dispatch.logMessage().contains("breakdown"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), () -> "Expected payload to parse successfully but got: " + parseResult.errorMessage());
        return parseResult.envelope();
    }
}
