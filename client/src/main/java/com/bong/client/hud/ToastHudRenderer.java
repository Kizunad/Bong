package com.bong.client.hud;

import com.bong.client.state.NarrationState;

import java.util.List;

final class ToastHudRenderer {
    private ToastHudRenderer() {
    }

    static boolean append(
        List<HudRenderCommand> commands,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxWidth,
        int x,
        int y
    ) {
        return BongToast.buildCommand(nowMillis, widthMeasurer, maxWidth)
            .map(command -> {
                commands.add(command);
                return true;
            })
            .orElse(false);
    }
}
