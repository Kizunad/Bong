package com.bong.client.hud;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** Aggregates Dugu v2 HUD surfaces without owning gameplay state. */
public final class DuguV2HudPlanner {
    private static final int BAR_WIDTH = 92;
    private static final int BAR_HEIGHT = 5;

    private DuguV2HudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        DuguV2HudStateStore.State state,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        DuguV2HudStateStore.State safeState = state == null ? DuguV2HudStateStore.State.NONE : state;
        if (screenWidth <= 0 || screenHeight <= 0) return List.of();

        List<HudRenderCommand> out = new ArrayList<>();
        if (safeState.tainted()) {
            out.add(HudRenderCommand.edgeVignette(HudRenderLayer.DUGU_TAINT_WARNING, 0x66305018));
            String hint = safeState.taintHint().isEmpty() ? "蛊毒" : safeState.taintHint();
            out.add(HudRenderCommand.text(
                HudRenderLayer.DUGU_TAINT_INDICATOR,
                hint,
                12,
                Math.max(12, screenHeight / 2 - 18),
                0x9BE15D
            ));
        }
        if (safeState.revealRisk() > 0f) {
            int x = Math.max(8, screenWidth - BAR_WIDTH - 12);
            int y = Math.max(12, screenHeight / 2 + 10);
            int filled = Math.max(1, Math.round(BAR_WIDTH * safeState.revealRisk()));
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_REVEAL_RISK, x, y, BAR_WIDTH, BAR_HEIGHT, 0x44202020));
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_REVEAL_RISK, x, y, filled, BAR_HEIGHT, 0xAA7FD447));
            out.add(HudRenderCommand.text(
                HudRenderLayer.DUGU_REVEAL_RISK,
                String.format(Locale.ROOT, "暴露 %.0f%%", safeState.revealRisk() * 100f),
                x,
                y - 10,
                0xC8F59A
            ));
        }
        if (safeState.selfCurePercent() > 0f || safeState.selfRevealed()) {
            int x = 12;
            int y = Math.max(24, screenHeight - 72);
            int filled = Math.max(1, Math.round(BAR_WIDTH * safeState.selfCurePercent() / 100f));
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_SELF_CURE_PROGRESS, x, y, BAR_WIDTH, BAR_HEIGHT, 0x44202020));
            out.add(HudRenderCommand.rect(HudRenderLayer.DUGU_SELF_CURE_PROGRESS, x, y, filled, BAR_HEIGHT, 0xAA4A8F2A));
            String suffix = safeState.selfRevealed() ? " 已露" : "";
            out.add(HudRenderCommand.text(
                HudRenderLayer.DUGU_SELF_CURE_PROGRESS,
                String.format(Locale.ROOT, "自蕴 %.1f%%%s", safeState.selfCurePercent(), suffix),
                x,
                y - 10,
                0xBCEB88
            ));
        }
        if (safeState.shroudActive() && safeState.shroudUntilMs() > nowMillis) {
            out.add(HudRenderCommand.screenTint(HudRenderLayer.DUGU_SHROUD, 0x20224516));
        }
        return List.copyOf(out);
    }
}
