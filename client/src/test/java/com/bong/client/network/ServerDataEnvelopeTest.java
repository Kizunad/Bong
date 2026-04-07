package com.bong.client.network;

import com.google.gson.JsonArray;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ServerDataEnvelopeTest {
    @Test
    void parsesLegacyWelcomeFixture() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-legacy-welcome.json");
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isSuccess(), "legacy welcome payload should parse successfully");
        ServerDataEnvelope envelope = result.envelope();
        assertNotNull(envelope);
        assertEquals(1, envelope.version());
        assertEquals("welcome", envelope.type());
        assertEquals("Bong server connected", envelope.message().orElseThrow());
    }

    @Test
    void parsesLegacyHeartbeatFixture() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-legacy-heartbeat.json");
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isSuccess(), "legacy heartbeat payload should parse successfully");
        ServerDataEnvelope envelope = result.envelope();
        assertNotNull(envelope);
        assertEquals(1, envelope.version());
        assertEquals("heartbeat", envelope.type());
        assertEquals("mock agent tick", envelope.message().orElseThrow());
    }

    @Test
    void parsesNestedNarrationFixture() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-nested-narration.json");
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isSuccess(), "nested narration payload should parse successfully");
        ServerDataEnvelope envelope = result.envelope();
        JsonArray narrations = envelope.payload().getAsJsonArray("narrations");
        assertNotNull(narrations);
        assertEquals(2, narrations.size());
        assertTrue(envelope.message().isEmpty(), "nested payload should not require legacy message field");
    }

    @Test
    void missingVersionReturnsError() throws IOException {
        String json = PayloadFixtureLoader.readText("missing-version-zone-info.json");
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Missing version 'v' field"));
    }

    @Test
    void unsupportedVersionReturnsError() throws IOException {
        String json = PayloadFixtureLoader.readText("wrong-version-player-state.json");
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Unsupported version"));
    }

    @Test
    void malformedJsonReturnsError() throws IOException {
        String json = PayloadFixtureLoader.readText("malformed-event-alert.json");
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Malformed JSON"));
    }

    @Test
    void invalidUtf8BytesDecodeWithReplacementCharacter() throws IOException {
        byte[] rawBytes = PayloadFixtureLoader.readHexBytes("invalid-utf8-welcome.hex");
        String json = ServerDataEnvelope.decodeUtf8(rawBytes);
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, rawBytes.length);

        assertTrue(json.contains("\uFFFD") || json.contains("�"));
        assertTrue(result.isSuccess(), "replacement-safe UTF-8 decoding should still allow valid JSON to parse");
        assertTrue(result.envelope().message().orElseThrow().contains("�("));
    }

    @Test
    void oversizePayloadReturnsError() throws IOException {
        String json = PayloadFixtureLoader.readText("oversize-ui-open.json");
        int payloadSizeBytes = json.getBytes(StandardCharsets.UTF_8).length;
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, payloadSizeBytes);

        assertTrue(payloadSizeBytes > ServerDataEnvelope.MAX_PAYLOAD_BYTES);
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Payload exceeds max size"));
    }
}
