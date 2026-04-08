package com.bong.client;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongNetworkHandlerTest {

    @Test
    public void testValidWelcomeJson() {
        String json = "{\"v\":1,\"type\":\"welcome\",\"message\":\"Bong server connected\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "Should successfully parse valid welcome JSON");
        assertNotNull(result.payload);
        BongServerPayload.WelcomePayload payload = assertInstanceOf(BongServerPayload.WelcomePayload.class, result.payload);
        assertEquals(1, payload.v());
        assertEquals("welcome", payload.type());
        assertEquals("Bong server connected", payload.message());
        assertTrue(BongServerPayloadRouter.route(null, payload));
    }

    @Test
    public void testValidHeartbeatJson() {
        String json = "{\"v\":1,\"type\":\"heartbeat\",\"message\":\"mock agent tick\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "Should successfully parse valid heartbeat JSON");
        assertNotNull(result.payload);
        BongServerPayload.HeartbeatPayload payload = assertInstanceOf(BongServerPayload.HeartbeatPayload.class, result.payload);
        assertEquals(1, payload.v());
        assertEquals("heartbeat", payload.type());
        assertEquals("mock agent tick", payload.message());
        assertTrue(BongServerPayloadRouter.route(null, payload));
    }

    @Test
    public void invalidJsonReturnsErrorResult() {
        String json = "{\"v\":1,\"type\":\"welcome\",broken json}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertFalse(result.success, "Should fail to parse malformed JSON");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Malformed JSON"));
    }

    @Test
    public void testWrongVersionReturnsError() {
        String json = "{\"v\":2,\"type\":\"welcome\",\"message\":\"future message\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertFalse(result.success, "Should fail when version is unsupported");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Unsupported version"));
    }

    @Test
    public void testMissingFieldsReturnsError() {
        String json = "{\"v\":1,\"type\":\"welcome\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertFalse(result.success, "Should fail when missing required fields");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Missing required field 'message'"));
    }

    @Test
    public void unknownTypeReturnsErrorResult() {
        String json = "{\"v\":1,\"type\":\"unknown_payload\",\"payload\":{}}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertFalse(result.success, "Should fail when payload type is unknown");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Unknown payload type"));
    }
}
