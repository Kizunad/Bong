package com.bong.client.visual.realm_vision;

import com.bong.client.visual.EdgeDecalRenderer;
import net.minecraft.client.gui.DrawContext;

public final class RealmVisionVignetteOverlay {
    private RealmVisionVignetteOverlay() {
    }

    public static void render(DrawContext context, int screenWidth, int screenHeight, long tick) {
        RealmVisionCommand command = RealmVisionPlanner.plan(RealmVisionStateStore.snapshot(), tick);
        if (command == null || command.vignetteAlpha() <= 0.0) {
            return;
        }
        int alpha = (int) Math.round(command.vignetteAlpha() * 255.0);
        EdgeDecalRenderer.render(context, screenWidth, screenHeight, (alpha << 24));
    }
}
