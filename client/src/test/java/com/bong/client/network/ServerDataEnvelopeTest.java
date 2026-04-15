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
    void fractionalVersionReturnsMalformedJsonError() {
        String json = """
            {"v":1.9,"type":"welcome","message":"Bong server connected"}
            """;
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isSuccess());
        assertEquals("Malformed JSON: field 'v' must be an integer", result.errorMessage());
    }

    @Test
    void integerVersionOneParsesSuccessfully() {
        String json = """
            {"v":1,"type":"heartbeat","message":"mock agent tick"}
            """;
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isSuccess(), "integer version token should parse successfully");
        assertEquals(1, result.envelope().version());
        assertEquals("heartbeat", result.envelope().type());
    }

    @Test
    void unsupportedIntegerVersionReturnsExactVersionError() {
        String json = """
            {"v":2,"type":"welcome","message":"Bong server connected"}
            """;
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isSuccess());
        assertEquals("Unsupported version: 2", result.errorMessage());
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
    void oversizePayloadReturnsError() {
        String json = buildWelcomePayloadOfSize(ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1);
        int payloadSizeBytes = json.getBytes(StandardCharsets.UTF_8).length;
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, payloadSizeBytes);

        assertEquals(ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1, payloadSizeBytes);
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Payload exceeds max size of 8192 bytes"));
    }

    @Test
    void acceptsPayloadExactlyAtClientBudget() {
        String json = buildWelcomePayloadOfSize(ServerDataEnvelope.MAX_PAYLOAD_BYTES);
        int payloadSizeBytes = json.getBytes(StandardCharsets.UTF_8).length;

        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, payloadSizeBytes);

        assertEquals(ServerDataEnvelope.MAX_PAYLOAD_BYTES, payloadSizeBytes);
        assertTrue(result.isSuccess(), "payload at the shared 8192-byte budget should parse successfully");
        assertEquals("welcome", result.envelope().type());
    }

    @Test
    void rejectsPayloadAboveClientBudget() {
        String json = buildWelcomePayloadOfSize(ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1);
        int payloadSizeBytes = json.getBytes(StandardCharsets.UTF_8).length;

        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, payloadSizeBytes);

        assertEquals(ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1, payloadSizeBytes);
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("Payload exceeds max size of 8192 bytes"));
    }

    private static String buildWelcomePayloadOfSize(int targetSizeBytes) {
        String prefix = "{\"v\":1,\"type\":\"welcome\",\"message\":\"";
        String suffix = "\"}";
        int messageLength = targetSizeBytes - prefix.length() - suffix.length();
        if (messageLength < 0) {
            throw new IllegalArgumentException("target size too small: " + targetSizeBytes);
        }

        return prefix + "a".repeat(messageLength) + suffix;
    }
}
