package com.bong.client.hud;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class CoffinHudPlanner {
    public static final String LABEL = "卧棺 · 寿火徐燃";
    public static final int PANEL_WIDTH = 148;
    public static final int PANEL_HEIGHT = 24;
    public static final int BOTTOM_MARGIN = 118;

    private static final int PANEL_COLOR = 0x8A101010;
    private static final int LABEL_COLOR = 0xB8D0D0D0;
    private static final int MULTIPLIER_COLOR = 0xA8B8C8D8;

    private CoffinHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        return buildCommands(CoffinStateStore.snapshot(), screenWidth, screenHeight);
    }

    static List<HudRenderCommand> buildCommands(
        CoffinStateStore.State state,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.inCoffin() || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        int x = (screenWidth - PANEL_WIDTH) / 2;
        int y = screenHeight - BOTTOM_MARGIN;
        out.add(HudRenderCommand.rect(
            HudRenderLayer.COFFIN,
            x,
            y,
            PANEL_WIDTH,
            PANEL_HEIGHT,
            PANEL_COLOR
        ));
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.COFFIN,
            LABEL,
            x + 9,
            y + 5,
            LABEL_COLOR,
            0.75
        ));
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.COFFIN,
            String.format(Locale.ROOT, "×%.1f", state.lifespanRateMultiplier()),
            x + PANEL_WIDTH - 36,
            y + 5,
            MULTIPLIER_COLOR,
            0.75
        ));
        return out;
    }
}
