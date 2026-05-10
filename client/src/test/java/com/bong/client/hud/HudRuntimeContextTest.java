package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class HudRuntimeContextTest {
    @Test
    void constructorDropsNullCompassMarkers() {
        List<HudRuntimeContext.CompassMarker> markers = new ArrayList<>();
        markers.add(null);
        markers.add(new HudRuntimeContext.CompassMarker(
            HudRuntimeContext.CompassMarker.Kind.SPIRIT_NICHE,
            10.0,
            20.0,
            1.0
        ));

        HudRuntimeContext context = new HudRuntimeContext(0.0, 0.0, 64.0, 0.0, false, markers);

        assertEquals(1, context.compassMarkers().size());
        assertEquals(HudRuntimeContext.CompassMarker.Kind.SPIRIT_NICHE, context.compassMarkers().get(0).kind());
    }
}
