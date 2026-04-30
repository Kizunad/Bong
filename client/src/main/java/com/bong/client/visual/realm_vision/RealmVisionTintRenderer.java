package com.bong.client.visual.realm_vision;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.List;

public final class RealmVisionTintRenderer {
    private RealmVisionTintRenderer() {
    }

    public static void append(List<HudRenderCommand> commands, RealmVisionCommand command) {
        if (commands == null || command == null || ((command.tintColorArgb() >>> 24) & 0xFF) == 0) {
            return;
        }
        commands.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, command.tintColorArgb()));
    }
}
