package com.bong.client.botany;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyPlantVisualStateTest {
    @Test
    void seedlingUsesSmallTransparentQuad() {
        BotanyPlantVisualState visual = BotanyPlantVisualState.forStage(
            PlantGrowthStage.SEEDLING,
            0x22FF44,
            0,
            0.0f
        );

        assertEquals(0.30f, visual.scale(), 1e-6);
        assertEquals(128, visual.alpha());
        assertEquals(0x22FF44, visual.tintRgb());
    }

    @Test
    void growingAddsSmallSway() {
        BotanyPlantVisualState early = BotanyPlantVisualState.forStage(
            PlantGrowthStage.GROWING,
            0x22FF44,
            0,
            0.0f
        );
        BotanyPlantVisualState later = BotanyPlantVisualState.forStage(
            PlantGrowthStage.GROWING,
            0x22FF44,
            10,
            0.5f
        );

        assertEquals(0.70f, early.scale(), 1e-6);
        assertNotEquals(early.swayRadians(), later.swayRadians());
        assertTrue(Math.abs(later.swayRadians()) <= 0.045f);
    }

    @Test
    void wiltedDesaturatesWithoutCrushingBrightness() {
        BotanyPlantVisualState visual = BotanyPlantVisualState.forStage(
            PlantGrowthStage.WILTED,
            0x22FF44,
            0,
            0.0f
        );

        assertEquals(0.92f, visual.scale(), 1e-6);
        assertEquals(220, visual.alpha());
        assertNotEquals(0x22FF44, visual.tintRgb());
        assertTrue(((visual.tintRgb() >> 16) & 0xFF) > 0);
    }
}
