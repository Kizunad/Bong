package com.bong.client.hud;

import com.bong.client.state.ZoneState;

import java.util.List;

final class ZoneHudRenderer {
    private ZoneHudRenderer() {
    }

    static boolean append(
        List<HudRenderCommand> commands,
        ZoneState zoneState,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxWidth,
        int x,
        int y
    ) {
        return append(commands, zoneState, 0L, widthMeasurer, maxWidth, x, y, 0, 0);
    }

    static boolean append(
        List<HudRenderCommand> commands,
        ZoneState zoneState,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxWidth,
        int x,
        int y,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> zoneCommands = BongZoneHud.buildCommands(
            zoneState,
            nowMillis,
            widthMeasurer,
            maxWidth,
            x,
            y,
            screenWidth,
            screenHeight
        );
        if (zoneCommands.isEmpty()) {
            return false;
        }

        commands.addAll(zoneCommands);
        return true;
    }
}
