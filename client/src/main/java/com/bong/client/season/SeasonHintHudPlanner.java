package com.bong.client.season;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.state.SeasonState;

import java.util.ArrayList;
import java.util.List;

public final class SeasonHintHudPlanner {
    static final int ICON_SIZE = 8;
    static final int ALPHA = 0x66000000;

    private SeasonHintHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        SeasonState state,
        int screenWidth,
        int screenHeight
    ) {
        int minWidth = 4 + ICON_SIZE;
        int minHeight = 10 + ICON_SIZE;
        if (state == null || screenWidth < minWidth || screenHeight < minHeight) {
            return List.of();
        }
        int x = Math.max(4, screenWidth - 18);
        int y = 10;
        List<HudRenderCommand> out = new ArrayList<>();
        switch (state.phase()) {
            case SUMMER -> appendSummerIcon(out, x, y);
            case WINTER -> appendWinterIcon(out, x, y);
            case SUMMER_TO_WINTER, WINTER_TO_SUMMER -> appendTideIcon(out, x, y, state.tickIntoPhase());
        }
        return List.copyOf(out);
    }

    private static void appendSummerIcon(List<HudRenderCommand> out, int x, int y) {
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x + 3, y, 2, 2, ALPHA | 0xFFB040));
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x + 2, y + 2, 4, 4, ALPHA | 0xE07020));
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x + 3, y + 5, 2, 3, ALPHA | 0xFFD060));
    }

    private static void appendWinterIcon(List<HudRenderCommand> out, int x, int y) {
        int white = ALPHA | 0xF0F8FF;
        int blue = ALPHA | 0x80C8FF;
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x + 3, y, 2, ICON_SIZE, white));
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x, y + 3, ICON_SIZE, 2, white));
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x + 2, y + 2, 4, 4, blue));
    }

    private static void appendTideIcon(List<HudRenderCommand> out, int x, int y, long tickIntoPhase) {
        int jitter = (int) ((tickIntoPhase / 6L) % 3L) - 1;
        int purple = ALPHA | 0x9966CC;
        int grey = ALPHA | 0xB0A8BC;
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x + 1 + jitter, y + 1, 6, 1, purple));
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x + 1 - jitter, y + 4, 6, 1, grey));
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x + 1 + jitter, y + 7, 6, 1, purple));
    }
}
