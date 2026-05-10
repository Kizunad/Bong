package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BaomaiV3VfxPlayerTest {
    @Test
    void hudSideEffectsOnlyAcceptLocalPlayerOrigin() {
        assertTrue(BaomaiV3VfxPlayer.isLocalPlayerOrigin(
            new double[] { 10.0, 64.0, 10.0 },
            new double[] { 10.5, 64.0, 10.5 }
        ));

        assertFalse(BaomaiV3VfxPlayer.isLocalPlayerOrigin(
            new double[] { 10.0, 64.0, 10.0 },
            new double[] { 16.0, 64.0, 10.0 }
        ));
    }
}
