package com.bong.client.alchemy;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class AlchemyFurnaceComponentTest {
    @Test
    void localClickStillHitsTheSamePartAfterWindowMoves() {
        int artX = 40;
        int artY = 18;
        int artWidth = 420;
        int artHeight = 280;
        double localX = artX + artWidth * (.40 + .30 / 2.0);
        double localY = artY + artHeight * (.28 + .16 / 2.0);

        int firstWindowX = 24;
        int secondWindowX = 312;
        double firstScreenX = firstWindowX + localX;
        double secondScreenX = secondWindowX + localX;
        assertEquals(
            AlchemyFurnaceComponent.Part.MOUTH,
            AlchemyFurnaceComponent.hitPart(
                artX, artY, artWidth, artHeight, firstScreenX - firstWindowX, localY));
        assertEquals(
            AlchemyFurnaceComponent.Part.MOUTH,
            AlchemyFurnaceComponent.hitPart(
                artX, artY, artWidth, artHeight, secondScreenX - secondWindowX, localY));
    }
}
