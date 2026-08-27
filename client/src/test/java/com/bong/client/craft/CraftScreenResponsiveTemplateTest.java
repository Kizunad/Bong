package com.bong.client.craft;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class CraftScreenResponsiveTemplateTest {
    @Test
    void minimumAndOddViewportsUseCompactTemplate() {
        assertEquals("craft-compact", CraftScreen.templateIdForViewport(320, 240));
        assertEquals("craft-compact", CraftScreen.templateIdForViewport(401, 241));
    }

    @Test
    void eitherNarrowDimensionKeepsCompactTemplate() {
        assertEquals("craft-compact", CraftScreen.templateIdForViewport(659, 360));
        assertEquals("craft-compact", CraftScreen.templateIdForViewport(660, 359));
    }

    @Test
    void wideBoundaryAndLargerViewportsUseThreeColumnTemplate() {
        assertEquals("craft", CraftScreen.templateIdForViewport(660, 360));
        assertEquals("craft", CraftScreen.templateIdForViewport(1920, 1080));
    }
}
