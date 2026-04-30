package com.bong.client.visual.realm_vision;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class RealmVisionPlannerTest {
    @Test
    void clampToRenderDistance() {
        RealmVisionCommand command = new RealmVisionCommand(240.0, 320.0, 0x7888A0, FogShape.SPHERE, 0.0, 0, 0.8, 0.5);
        RealmVisionCommand clamped = RealmVisionPlanner.clampToRenderDistance(command, 12);
        assertEquals(188.0, clamped.fogStart());
        assertEquals(188.0, clamped.fogEnd());
    }

    @Test
    void planUsesTickOffsetFromPayloadStart() {
        RealmVisionCommand oldCommand = new RealmVisionCommand(30.0, 60.0, 0, FogShape.CYLINDER, 0.5, 0, 0, 0);
        RealmVisionState state = new RealmVisionState(
            new RealmVisionCommand(130.0, 160.0, 0xFFFFFF, FogShape.SPHERE, 0.0, 0, 1, 1),
            oldCommand,
            100,
            0,
            50,
            8
        );
        assertEquals(80.0, RealmVisionPlanner.plan(state, 100).fogStart());
    }

    @Test
    void planIgnoresFrameAdvancedElapsedTicks() {
        RealmVisionCommand oldCommand = new RealmVisionCommand(30.0, 60.0, 0, FogShape.CYLINDER, 0.5, 0, 0, 0);
        RealmVisionState state = new RealmVisionState(
            new RealmVisionCommand(130.0, 160.0, 0xFFFFFF, FogShape.SPHERE, 0.0, 0, 1, 1),
            oldCommand,
            100,
            90,
            50,
            8
        );
        assertEquals(40.0, RealmVisionPlanner.plan(state, 60).fogStart());
    }
}
