package com.bong.client.visual.realm_vision;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class MixinBgRendererSinkTest {
    @Test
    void sinkReceivesPlannerOutput() {
        List<RealmVisionCommand> applied = new ArrayList<>();
        RealmVisionFogController.setSinkForTests(applied::add);
        RealmVisionStateStore.replace(new RealmVisionState(
            new RealmVisionCommand(30.0, 60.0, 0xB8B0A8, FogShape.CYLINDER, 0.55, 0, 0, 0),
            null,
            0,
            0,
            0,
            4
        ));
        RealmVisionFogController.apply(1);
        RealmVisionFogController.apply(2);
        assertEquals(2, applied.size());
        assertEquals(30.0, applied.get(0).fogStart());
        RealmVisionFogController.setSinkForTests(null);
    }
}
