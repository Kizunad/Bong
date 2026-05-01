package com.bong.client.visual.realm_vision;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PerceptionEdgeProjectorTest {
    @Test
    void insideViewDoesNotDrawEdge() {
        EdgeIndicatorCmd cmd = PerceptionEdgeProjector.project(0, 64, 10, 0, 64, 0, 0, 0, 70, 320, 180, SenseKind.LIVING_QI, 0.5);
        assertFalse(cmd.onEdge());
        assertEquals(160, cmd.x());
        assertEquals(90, cmd.y());
    }

    @Test
    void projectsLeftAndRightEdges() {
        EdgeIndicatorCmd left = PerceptionEdgeProjector.project(-100, 64, 10, 0, 64, 0, 0, 0, 70, 320, 180, SenseKind.LIVING_QI, 1.0);
        EdgeIndicatorCmd right = PerceptionEdgeProjector.project(100, 64, 10, 0, 64, 0, 0, 0, 70, 320, 180, SenseKind.LIVING_QI, 1.0);
        assertTrue(left.onEdge());
        assertTrue(right.onEdge());
        assertEquals(DirectionBucket.LEFT, left.bucket());
        assertEquals(DirectionBucket.RIGHT, right.bucket());
    }

    @Test
    void projectsBehindCameraToEdge() {
        EdgeIndicatorCmd cmd = PerceptionEdgeProjector.project(0, 64, -10, 0, 64, 0, 0, 0, 70, 320, 180, SenseKind.CRISIS_PREMONITION, 1.0);
        assertTrue(cmd.onEdge());
    }

    @Test
    void priorityOverflowKeepsThreePerDirection() {
        List<EdgeIndicatorCmd> capped = PerceptionEdgeProjector.capPerDirection(List.of(
            new EdgeIndicatorCmd(1, 1, SenseKind.LIVING_QI, 0.1, true, DirectionBucket.LEFT),
            new EdgeIndicatorCmd(1, 2, SenseKind.LIVING_QI, 0.9, true, DirectionBucket.LEFT),
            new EdgeIndicatorCmd(1, 3, SenseKind.LIVING_QI, 0.8, true, DirectionBucket.LEFT),
            new EdgeIndicatorCmd(1, 4, SenseKind.LIVING_QI, 0.7, true, DirectionBucket.LEFT)
        ));
        assertEquals(3, capped.size());
        assertEquals(0.9, capped.get(0).intensity());
        assertEquals(0.7, capped.get(2).intensity());
    }
}
