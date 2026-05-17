package com.bong.client.iris;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class ShaderStateHandlerTest {
    @BeforeEach
    void setUp() {
        BongShaderState.reset();
    }

    @Test
    void parsesValidPayload() {
        String json = "{\"bong_bloodmoon\": 0.8, \"bong_wind_strength\": 0.5, \"bong_wind_angle\": 3.14}";
        boolean result = ShaderStateHandler.handle(json);
        assertTrue(result, "Should return true for valid payload");
        assertEquals(0.8f, BongShaderState.getTarget(BongUniform.BLOODMOON), 0.001f);
        assertEquals(0.5f, BongShaderState.getTarget(BongUniform.WIND_STRENGTH), 0.001f);
        assertEquals(3.14f, BongShaderState.getTarget(BongUniform.WIND_ANGLE), 0.01f,
                "WIND_ANGLE target uses per-uniform clamp (0-2*PI range)");
    }

    @Test
    void ignoresUnknownFields() {
        String json = "{\"bong_bloodmoon\": 0.5, \"bong_unknown\": 0.9, \"extra\": true}";
        boolean result = ShaderStateHandler.handle(json);
        assertTrue(result);
        assertEquals(0.5f, BongShaderState.getTarget(BongUniform.BLOODMOON), 0.001f);
    }

    @Test
    void handlesEmptyObject() {
        boolean result = ShaderStateHandler.handle("{}");
        assertTrue(result, "Empty object is valid (no uniforms changed)");
    }

    @Test
    void rejectsNullPayload() {
        assertFalse(ShaderStateHandler.handle(null));
    }

    @Test
    void rejectsEmptyString() {
        assertFalse(ShaderStateHandler.handle(""));
    }

    @Test
    void rejectsNonObject() {
        assertFalse(ShaderStateHandler.handle("[1,2,3]"));
        assertFalse(ShaderStateHandler.handle("\"hello\""));
    }

    @Test
    void rejectsInvalidJson() {
        assertFalse(ShaderStateHandler.handle("{broken json"));
    }

    @Test
    void partialPayloadOnlyUpdatesPresent() {
        BongShaderState.setTarget(BongUniform.REALM, 0.7f);
        String json = "{\"bong_meditation\": 0.3}";
        ShaderStateHandler.handle(json);
        assertEquals(0.7f, BongShaderState.getTarget(BongUniform.REALM), 0.001f,
                "Uniforms not in payload should retain their previous target");
        assertEquals(0.3f, BongShaderState.getTarget(BongUniform.MEDITATION), 0.001f);
    }

    @Test
    void nonNumericValuesIgnored() {
        String json = "{\"bong_bloodmoon\": \"hello\", \"bong_demonic\": 0.6}";
        boolean result = ShaderStateHandler.handle(json);
        assertTrue(result);
        assertEquals(0f, BongShaderState.getTarget(BongUniform.BLOODMOON), 0.001f,
                "Non-numeric value should not update the uniform");
        assertEquals(0.6f, BongShaderState.getTarget(BongUniform.DEMONIC), 0.001f);
    }
}
