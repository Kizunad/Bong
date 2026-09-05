package com.bong.client.hud;

import com.bong.client.state.ZoneState;

import java.util.ArrayList;
import java.util.List;

/** 仅保留切区标题和跨维度黑场，不再生成区域常驻面板。 */
public final class ZoneTransitionHudPlanner {
    private static final long TITLE_HOLD_MS = 2_000L;
    private static final long TITLE_DURATION_MS = 3_000L;
    private static final long BLACKOUT_DURATION_MS = 500L;

    private ZoneTransitionHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        ZoneState state, long nowMillis, HudTextHelper.WidthMeasurer measurer, int width, int height
    ) {
        if (state == null || state.isEmpty() || measurer == null || width <= 0 || height <= 0) {
            return List.of();
        }
        long elapsed = Math.max(0L, Math.max(0L, nowMillis) - Math.max(0L, state.changedAtMillis()));
        if (elapsed >= TITLE_DURATION_MS) {
            return List.of();
        }
        List<HudRenderCommand> commands = new ArrayList<>();
        if (state.dimensionTransition() && elapsed < BLACKOUT_DURATION_MS) {
            int alpha = (int) Math.round(220.0 * (BLACKOUT_DURATION_MS - elapsed) / BLACKOUT_DURATION_MS);
            commands.add(HudRenderCommand.screenTint(HudRenderLayer.ZONE_TRANSITION, alpha << 24));
        }
        String title = "— " + state.zoneLabel() + (state.negativeSpiritQi() ? " ⚠ 负灵域" : "") + " —";
        String clipped = HudTextHelper.clipToWidth(title, Math.max(1, width - 16), measurer);
        if (!clipped.isEmpty()) {
            int alpha = elapsed <= TITLE_HOLD_MS ? 255
                : (int) Math.round(255.0 * (TITLE_DURATION_MS - elapsed) / (TITLE_DURATION_MS - TITLE_HOLD_MS));
            int color = state.negativeSpiritQi() ? 0xEE6677 : 0xFFD700;
            commands.add(HudRenderCommand.text(HudRenderLayer.ZONE_TRANSITION, clipped,
                Math.max(0, (width - measurer.measure(clipped)) / 2), height / 3,
                HudTextHelper.withAlpha(color, alpha)));
        }
        return List.copyOf(commands);
    }
}
