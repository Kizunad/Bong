package com.bong.client.network;

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
    }

    @Test
    public void testValidHeartbeatJson() {
        String json = "{\"v\":1,\"type\":\"heartbeat\",\"message\":\"mock agent tick\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertTrue(result.success, "Should successfully parse valid heartbeat JSON");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("heartbeat", result.payload.type);
    }

    @Test
    public void testValidNarrationJson() {
        String json = "{\"v\":1,\"type\":\"narration\",\"narrations\":[]}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertTrue(result.success, "Should successfully parse valid narration JSON");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("narration", result.payload.type);
    }

    @Test
    public void testValidZoneInfoJson() {
        String json = "{\"v\":1,\"type\":\"zone_info\",\"zone\":\"spawn\",\"spirit_qi\":0.5,\"danger_level\":0}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertTrue(result.success, "Should successfully parse valid zone_info JSON");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("zone_info", result.payload.type);
    }

    @Test
    public void testValidEventAlertJson() {
        String json = "{\"v\":1,\"type\":\"event_alert\",\"event\":\"thunder_tribulation\",\"message\":\"Alert!\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertTrue(result.success, "Should successfully parse valid event_alert JSON");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("event_alert", result.payload.type);
    }

    @Test
    public void testValidPlayerStateJson() {
        String json = "{\"v\":1,\"type\":\"player_state\",\"realm\":\"qi_refining_3\",\"spirit_qi\":78,\"karma\":0.2,\"composite_power\":0.35,\"breakdown\":{\"combat\":0.2,\"wealth\":0.4,\"social\":0.65,\"karma\":0.2,\"territory\":0.1},\"zone\":\"spawn\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertTrue(result.success, "Should successfully parse valid player_state JSON");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("player_state", result.payload.type);
    }

    @Test
    public void testUnknownNestedObjectFieldIsSkipped() {
        String json = "{\"v\":1,\"type\":\"welcome\",\"unknown\":{\"details\":{\"path\":[1,{\"leaf\":true},[\"qi\"]]},\"ignored\":null},\"message\":\"Bong server connected\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "Should successfully skip unknown nested object fields");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("welcome", result.payload.type);
    }

    @Test
    public void testUnknownArrayFieldIsSkipped() {
        String json = "{\"v\":1,\"type\":\"heartbeat\",\"extra\":[0,{\"nested\":{\"realm\":\"Qi\"}},[true,false,null]],\"message\":\"mock agent tick\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertTrue(result.success, "Should successfully skip unknown array fields");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("heartbeat", result.payload.type);
    }

    @Test
    public void testMalformedNestedObjectReturnsError() {
        String json = "{\"v\":1,\"type\":\"welcome\",\"unknown\":{\"details\":[1,2},\"message\":\"Bong server connected\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertFalse(result.success, "Should fail to parse malformed nested object JSON");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Malformed JSON"));
    }

    @Test
    public void testMalformedNestedArrayReturnsError() {
        String json = "{\"v\":1,\"type\":\"heartbeat\",\"extra\":[{\"nested\":true],\"message\":\"mock agent tick\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);

        assertFalse(result.success, "Should fail to parse malformed nested array JSON");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Malformed JSON"));
    }

    @Test
    public void testValidUiOpenJson() {
        String json = "{\"v\":1,\"type\":\"ui_open\",\"xml\":\"<ui></ui>\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertTrue(result.success, "Should successfully parse valid ui_open JSON");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("ui_open", result.payload.type);
    }

    @Test
    public void testUnknownTypeHandledGracefully() {
        String json = "{\"v\":1,\"type\":\"unknown_magic_type\"}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertTrue(result.success, "Should successfully parse unknown type (router will handle ignore)");
        assertNotNull(result.payload);
        assertEquals(1, result.payload.v);
        assertEquals("unknown_magic_type", result.payload.type);
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
        String json = "{\"v\":1}";
        BongNetworkHandler.ParseResult result = BongNetworkHandler.parseServerPayload(json);
        
        assertFalse(result.success, "Should fail when missing required field 'type'");
        assertNull(result.payload);
        assertNotNull(result.errorMessage);
        assertTrue(result.errorMessage.contains("Missing required field"));
    }
}
