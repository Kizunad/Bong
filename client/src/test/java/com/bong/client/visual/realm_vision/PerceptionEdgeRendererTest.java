package com.bong.client.visual.realm_vision;

import com.bong.client.hud.HudRenderCommand;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PerceptionEdgeRendererTest {
    @Test
    void rendersWithPerDirectionLimit() {
        List<HudRenderCommand> out = new ArrayList<>();
        List<EdgeIndicatorCmd> indicators = new ArrayList<>();
        int x = 10;
        for (SenseKind kind : SenseKind.values()) {
            indicators.add(new EdgeIndicatorCmd(x++, 10, kind, 0.8, true, DirectionBucket.TOP));
        }
        PerceptionEdgeRenderer.append(out, indicators);
        assertEquals(3, out.size());
        assertTrue(out.stream().allMatch(HudRenderCommand::isEdgeIndicator));
    }

    @Test
    void colorAlphaScalesWithIntensity() {
        int low = PerceptionEdgeRenderer.colorFor(SenseKind.LIVING_QI, 0.0) >>> 24;
        int high = PerceptionEdgeRenderer.colorFor(SenseKind.LIVING_QI, 1.0) >>> 24;
        assertTrue(high > low);
    }
}
