package com.bong.client;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

public class BongNetworkHandlerTest {

    @Test
    public void testValidWelcomeJson() {
        String json = "{\"v\":1,\"type\":\"welcome\",\"message\":\"Bong server connected\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertTrue(result.success, "Should successfully parse valid welcome JSON");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("welcome", result.payload.type);
        assertEquals("Bong server connected", result.payload.message);
    }

    @Test
    public void testValidHeartbeatJson() {
        String json = "{\"v\":1,\"type\":\"heartbeat\",\"message\":\"mock agent tick\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertTrue(result.success, "Should successfully parse valid heartbeat JSON");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("heartbeat", result.payload.type);
        assertEquals("mock agent tick", result.payload.message);
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
        assertTrue(result.errorMessage.contains("Missing required fields"));
    }
}
