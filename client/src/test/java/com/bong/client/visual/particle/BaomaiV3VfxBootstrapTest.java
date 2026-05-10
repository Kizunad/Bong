package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertTrue;

public class BaomaiV3VfxBootstrapTest {
    @Test
    void bootstrapRegistersAllBaomaiV3ParticleRecipes() {
        VfxRegistry.instance().clearForTests();

        VfxBootstrap.registerDefaults();

        assertTrue(VfxRegistry.instance().contains(BaomaiV3VfxPlayer.GROUND_WAVE_DUST));
        assertTrue(VfxRegistry.instance().contains(BaomaiV3VfxPlayer.BLOOD_BURN_CRIMSON));
        assertTrue(VfxRegistry.instance().contains(BaomaiV3VfxPlayer.BODY_TRANSCENDENCE_PILLAR));
        assertTrue(VfxRegistry.instance().contains(BaomaiV3VfxPlayer.MERIDIAN_RIPPLE_SCAR));
    }
}
