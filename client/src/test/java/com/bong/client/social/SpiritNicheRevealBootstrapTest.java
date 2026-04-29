package com.bong.client.social;

import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SpiritNicheRevealBootstrapTest {
    @AfterEach
    void tearDown() {
        SpiritNicheRevealBootstrap.resetForTests();
    }

    @Test
    void observeBlockReturnsTrueOnceAfterThreeSeconds() {
        BlockPos pos = new BlockPos(11, 64, 10);
        for (int i = 0; i < 59; i++) {
            assertFalse(SpiritNicheRevealBootstrap.observeBlockForTests(pos));
        }
        assertTrue(SpiritNicheRevealBootstrap.observeBlockForTests(pos));
        assertFalse(SpiritNicheRevealBootstrap.observeBlockForTests(pos));
    }

    @Test
    void observeBlockResetsWhenTargetChanges() {
        BlockPos first = new BlockPos(11, 64, 10);
        BlockPos second = new BlockPos(12, 64, 10);
        for (int i = 0; i < 40; i++) {
            assertFalse(SpiritNicheRevealBootstrap.observeBlockForTests(first));
        }
        assertFalse(SpiritNicheRevealBootstrap.observeBlockForTests(second));
        for (int i = 0; i < 58; i++) {
            assertFalse(SpiritNicheRevealBootstrap.observeBlockForTests(second));
        }
        assertTrue(SpiritNicheRevealBootstrap.observeBlockForTests(second));
    }
}
