package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class HudAnimationTest {
    @Test
    void toastSlideAnimationMovesInAndOut() {
        assertTrue(HudAnimation.toastSlideOffset(1_000L, 5_000L, 1_000L, 28) > 0);
        assertEquals(0, HudAnimation.toastSlideOffset(1_000L, 5_000L, 1_400L, 28));
        assertTrue(HudAnimation.toastSlideOffset(1_000L, 5_000L, 4_900L, 28) > 0);
    }

    @Test
    void progressBarSmoothLerpMovesTowardTarget() {
        assertEquals(30, HudAnimation.smoothFillWidth(0.0, 1.0, 100, 0.3));
        assertEquals("飞", HudAnimation.typewriterText("飞剑出鞘", 1_000L, 1_000L));
        assertEquals("飞剑", HudAnimation.typewriterText("飞剑出鞘", 1_000L, 1_030L));
    }
}
