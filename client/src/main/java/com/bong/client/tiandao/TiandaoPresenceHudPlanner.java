package com.bong.client.tiandao;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import java.util.ArrayList;
import java.util.List;

public final class TiandaoPresenceHudPlanner {
    private TiandaoPresenceHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        TiandaoPresenceState state,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        if (state == null || !state.active()) {
            return List.of();
        }
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, state.vignetteArgb(nowMillis)));
        int tint = state.tintArgb();
        if (tint != 0) {
            out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, tint));
        }
        if (state.shakeIntensity() > 0.0) {
            int alpha = (int) Math.round(48.0 * state.shakeIntensity());
            out.add(HudRenderCommand.rect(HudRenderLayer.EDGE_FEEDBACK, 0, 0, screenWidth, 1, (alpha << 24) | 0x601000));
            out.add(HudRenderCommand.rect(HudRenderLayer.EDGE_FEEDBACK, 0, screenHeight - 1, screenWidth, 1, (alpha << 24) | 0x601000));
        }
        return out;
    }
}
