package com.bong.client.hud;

import com.bong.client.BongClientFeatures;
import com.bong.client.state.VisualEffectState;
import com.bong.client.visual.VisualEffectPlanner;

import java.util.List;

final class VisualHudRenderer {
    private VisualHudRenderer() {
    }

    static boolean append(
        List<HudRenderCommand> commands,
        VisualEffectState visualEffectState,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> visualCommands = VisualEffectPlanner.buildCommands(
            visualEffectState,
            nowMillis,
            widthMeasurer,
            maxTextWidth,
            screenWidth,
            screenHeight,
            BongClientFeatures.ENABLE_VISUAL_EFFECTS
        );
        if (visualCommands.isEmpty()) {
            return false;
        }

        commands.addAll(visualCommands);
        return true;
    }
}
