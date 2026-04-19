package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyProjectionTest {
    private static final double FOV = 70.0;
    private static final int SCALED_W = 960;
    private static final int SCALED_H = 540;

    @Test
    void pointStraightAheadProjectsToScreenCenter() {
        BotanyProjection.Anchor anchor = BotanyProjection.project(
            0.0, 64.0, 10.0,
            0.0, 64.0, 0.0,
            0.0f, 0.0f,
            FOV,
            SCALED_W, SCALED_H
        );
        assertTrue(anchor.visible());
        assertEquals(SCALED_W / 2, anchor.x(), 1);
        assertEquals(SCALED_H / 2, anchor.y(), 1);
    }

    @Test
    void pointToTheRightProjectsRightOfCenter() {
        // Camera facing +Z (south, yaw=0), point at (+X, same y, +Z) should appear to the right of center.
        BotanyProjection.Anchor anchor = BotanyProjection.project(
            2.0, 64.0, 10.0,
            0.0, 64.0, 0.0,
            0.0f, 0.0f,
            FOV,
            SCALED_W, SCALED_H
        );
        assertTrue(anchor.visible());
        assertTrue(anchor.x() > SCALED_W / 2);
    }

    @Test
    void pointAboveProjectsAboveCenter() {
        BotanyProjection.Anchor anchor = BotanyProjection.project(
            0.0, 66.0, 10.0,
            0.0, 64.0, 0.0,
            0.0f, 0.0f,
            FOV,
            SCALED_W, SCALED_H
        );
        assertTrue(anchor.visible());
        assertTrue(anchor.y() < SCALED_H / 2);
    }

    @Test
    void pointBehindCameraIsInvisible() {
        BotanyProjection.Anchor anchor = BotanyProjection.project(
            0.0, 64.0, -10.0,
            0.0, 64.0, 0.0,
            0.0f, 0.0f,
            FOV,
            SCALED_W, SCALED_H
        );
        assertFalse(anchor.visible());
    }

    @Test
    void pointSouthWhenFacingWestIsBehind() {
        // yaw=90 → 面朝西（-X）。Point at (+X, 64, 0) should be behind.
        BotanyProjection.Anchor anchor = BotanyProjection.project(
            5.0, 64.0, 0.0,
            0.0, 64.0, 0.0,
            90.0f, 0.0f,
            FOV,
            SCALED_W, SCALED_H
        );
        assertFalse(anchor.visible());
    }

    @Test
    void zeroDimensionsReturnInvisible() {
        BotanyProjection.Anchor anchor = BotanyProjection.project(
            0, 0, 10,
            0, 0, 0,
            0.0f, 0.0f,
            FOV,
            0, 0
        );
        assertFalse(anchor.visible());
    }
}
