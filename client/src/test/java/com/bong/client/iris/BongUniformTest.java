package com.bong.client.iris;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class BongUniformTest {
    @Test
    void allUniformsHaveBongPrefix() {
        for (BongUniform u : BongUniform.values()) {
            assertTrue(u.shaderName().startsWith("bong_"),
                    "Uniform " + u.name() + " shader name should start with 'bong_', got: " + u.shaderName());
        }
    }

    @Test
    void shaderNameMatchesLowercaseName() {
        assertEquals("bong_realm", BongUniform.REALM.shaderName());
        assertEquals("bong_lingqi", BongUniform.LINGQI.shaderName());
        assertEquals("bong_tribulation", BongUniform.TRIBULATION.shaderName());
        assertEquals("bong_enlightenment", BongUniform.ENLIGHTENMENT.shaderName());
        assertEquals("bong_inkwash", BongUniform.INKWASH.shaderName());
        assertEquals("bong_bloodmoon", BongUniform.BLOODMOON.shaderName());
        assertEquals("bong_meditation", BongUniform.MEDITATION.shaderName());
        assertEquals("bong_demonic", BongUniform.DEMONIC.shaderName());
        assertEquals("bong_wind_strength", BongUniform.WIND_STRENGTH.shaderName());
        assertEquals("bong_wind_angle", BongUniform.WIND_ANGLE.shaderName());
    }

    @Test
    void fromShaderNameReturnsCorrectEnum() {
        for (BongUniform u : BongUniform.values()) {
            assertSame(u, BongUniform.fromShaderName(u.shaderName()),
                    "fromShaderName should round-trip for " + u.name());
        }
    }

    @Test
    void fromShaderNameReturnsNullForUnknown() {
        assertNull(BongUniform.fromShaderName("bong_nonexistent"));
        assertNull(BongUniform.fromShaderName(""));
        assertNull(BongUniform.fromShaderName("realm"));
        assertNull(BongUniform.fromShaderName(null));
    }

    @Test
    void enumHasExpectedCount() {
        assertEquals(10, BongUniform.values().length,
                "Expected 10 BongUniform variants (realm, lingqi, tribulation, enlightenment, inkwash, bloodmoon, meditation, demonic, wind_strength, wind_angle)");
    }
}
