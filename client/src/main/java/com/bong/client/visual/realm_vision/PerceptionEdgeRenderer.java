package com.bong.client.visual.realm_vision;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.List;

public final class PerceptionEdgeRenderer {
    private PerceptionEdgeRenderer() {
    }

    public static void append(List<HudRenderCommand> out, List<EdgeIndicatorCmd> indicators) {
        if (out == null || indicators == null || indicators.isEmpty()) {
            return;
        }
        // Spiritual sense v1.1 only draws off-screen edge markers; in-FOV targets remain visual-only.
        for (EdgeIndicatorCmd indicator : PerceptionEdgeProjector.capPerDirection(indicators)) {
            out.add(HudRenderCommand.edgeIndicator(
                HudRenderLayer.SPIRITUAL_SENSE,
                indicator.kind().name(),
                indicator.x(),
                indicator.y(),
                colorFor(indicator.kind(), indicator.intensity()),
                indicator.intensity()
            ));
        }
    }

    public static int colorFor(SenseKind kind, double intensity) {
        int base = switch (kind == null ? SenseKind.LIVING_QI : kind) {
            case LIVING_QI -> 0xF4F4FF;
            case AMBIENT_LEYLINE -> 0x65B9FF;
            case CULTIVATOR_REALM -> 0xD9A7FF;
            case HEAVENLY_GAZE -> 0xFF4A4A;
            case CRISIS_PREMONITION -> 0xFF2020;
        };
        int alpha = (int) Math.round(80.0 + Math.max(0.0, Math.min(1.0, intensity)) * 175.0);
        return (alpha << 24) | base;
    }
}
