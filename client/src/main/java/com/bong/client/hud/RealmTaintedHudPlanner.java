package com.bong.client.hud;

import java.util.List;

public final class RealmTaintedHudPlanner {
    private static final int NICHE_INTRUSION_COLOR = 0xFF5F626A;

    private RealmTaintedHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(float nicheIntrusionSeverity, int screenWidth) {
        float severity = Math.max(0.0f, Math.min(1.0f, nicheIntrusionSeverity));
        if (severity <= 0.0f) {
            return List.of();
        }
        String label = severity >= 1.0f ? "龛侵主色" : "龛侵色 " + Math.round(severity * 100.0f) + "%";
        return List.of(HudRenderCommand.text(
            HudRenderLayer.STATUS_EFFECTS,
            label,
            Math.max(8, screenWidth - 82),
            10,
            NICHE_INTRUSION_COLOR
        ));
    }
}
