package com.bong.client.tsy;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudTextHelper;

import java.util.ArrayList;
import java.util.List;

public final class TsyBossHealthBar {
    private static final int HEIGHT = 10;
    private static final int BG = 0xCC101018;
    private static final int FILL = 0xFFD24A4A;
    private static final int FLASH = 0xFFFFFFFF;
    private static final int TEXT = 0xFFECE8D8;
    private static final int BORDER = 0xAAE8D090;

    private TsyBossHealthBar() {
    }

    public static List<HudRenderCommand> buildCommands(
        TsyBossHealthState state,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth
    ) {
        TsyBossHealthState safe = state == null ? TsyBossHealthState.empty() : state;
        if (!safe.active() || screenWidth <= 0 || widthMeasurer == null) {
            return List.of();
        }
        int width = Math.max(180, screenWidth - 48);
        int x = (screenWidth - width) / 2;
        int y = 10;
        List<HudRenderCommand> out = new ArrayList<>();
        String title = HudTextHelper.clipToWidth(safe.bossName() + " · " + safe.realm(), width, widthMeasurer);
        out.add(HudRenderCommand.text(HudRenderLayer.TARGET_INFO, title, x, y, TEXT));
        int barY = y + 13;
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, barY, width, HEIGHT, BG));
        int fill = Math.max(0, Math.min(width, (int) Math.round(width * safe.healthRatio())));
        if (fill > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, barY, fill, HEIGHT, FILL));
        }
        appendPhaseSeparators(out, x, barY, width, safe.maxPhase(), separatorColor(safe, nowMillis));
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, barY, width, 1, BORDER));
        return List.copyOf(out);
    }

    static void appendPhaseSeparators(List<HudRenderCommand> out, int x, int y, int width, int maxPhase, int color) {
        for (int phase = 1; phase < maxPhase; phase++) {
            int sx = x + (int) Math.round(width * (phase / (double) maxPhase));
            out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, sx, y - 2, 1, HEIGHT + 4, color));
        }
    }

    private static int separatorColor(TsyBossHealthState state, long nowMillis) {
        long age = Math.max(0L, nowMillis - state.updatedAtMillis());
        if (age < 300L) {
            int alpha = (int) Math.round(255.0 * (1.0 - age / 300.0));
            return (alpha << 24) | (FLASH & 0x00FFFFFF);
        }
        return BORDER;
    }
}
