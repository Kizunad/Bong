package com.bong.client.hud;

import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;

import java.util.ArrayList;
import java.util.List;

public final class MovementHudPlanner {
    public static final long HOVER_VISIBLE_MS = 3_000L;
    public static final long HOVER_FADE_MS = 500L;
    public static final long REJECT_FLASH_MS = 300L;
    public static final int PANEL_WIDTH = 132;
    public static final int PANEL_HEIGHT = 28;
    public static final int BOTTOM_MARGIN = 86;

    private static final long DASH_COOLDOWN_MAX_TICKS = 40L;
    private static final long SLIDE_COOLDOWN_MAX_TICKS = 60L;
    private static final int TRACK_COLOR = 0xA0182228;
    private static final int DASH_COLOR = 0xFF9FD3FF;
    private static final int SLIDE_COLOR = 0xFFD7A56A;
    private static final int STAMINA_COLOR = 0xFF75E39A;
    private static final int STAMINA_LOW_COLOR = 0xFFE06040;
    private static final int DOT_READY_COLOR = 0xFFC8CCFF;
    private static final int DOT_SPENT_COLOR = 0xFF566070;
    private static final int REJECT_COLOR = 0xC0FF3030;

    private MovementHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight, long nowMs) {
        return buildCommands(MovementStateStore.snapshot(), screenWidth, screenHeight, nowMs);
    }

    static List<HudRenderCommand> buildCommands(
        MovementState state,
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        appendZoneFeedback(out, state.zoneKind());

        double alpha = hudAlpha(state, nowMs);
        if (alpha <= 0.0) {
            return out;
        }

        int x = (screenWidth - PANEL_WIDTH) / 2;
        int y = screenHeight - BOTTOM_MARGIN;
        out.add(HudRenderCommand.rect(
            HudRenderLayer.MOVEMENT_HUD,
            x,
            y,
            PANEL_WIDTH,
            PANEL_HEIGHT,
            withAlpha(TRACK_COLOR, alpha)
        ));
        appendCooldown(out, x + 6, y + 6, 48, state.dashCooldownRemainingTicks(), DASH_COOLDOWN_MAX_TICKS, DASH_COLOR, alpha);
        appendCooldown(out, x + 6, y + 15, 48, state.slideCooldownRemainingTicks(), SLIDE_COOLDOWN_MAX_TICKS, SLIDE_COLOR, alpha);
        out.add(HudRenderCommand.scaledText(HudRenderLayer.MOVEMENT_HUD, "DASH", x + 58, y + 3, withAlpha(DASH_COLOR, alpha), 0.6));
        out.add(HudRenderCommand.scaledText(HudRenderLayer.MOVEMENT_HUD, "SLIDE", x + 58, y + 12, withAlpha(SLIDE_COLOR, alpha), 0.6));

        appendStamina(out, state, x + 88, y + 20, alpha, nowMs);
        appendDoubleJumpDots(out, state, x + 92, y + 6, alpha);

        if (state.rejectedRecently(nowMs, REJECT_FLASH_MS)) {
            out.add(HudRenderCommand.rect(
                HudRenderLayer.MOVEMENT_HUD,
                x,
                y,
                PANEL_WIDTH,
                PANEL_HEIGHT,
                withAlpha(REJECT_COLOR, 1.0)
            ));
        }
        return out;
    }

    static double hudAlpha(MovementState state, long nowMs) {
        if (state.lowStamina()) {
            return Math.max(0.4, timedAlpha(state, nowMs));
        }
        return timedAlpha(state, nowMs);
    }

    private static double timedAlpha(MovementState state, long nowMs) {
        if (state.hudActivityAtMs() <= 0L || nowMs < state.hudActivityAtMs()) {
            return state.action() == MovementState.Action.NONE ? 0.0 : 1.0;
        }
        long elapsed = nowMs - state.hudActivityAtMs();
        if (state.action() != MovementState.Action.NONE || elapsed <= HOVER_VISIBLE_MS) {
            return 1.0;
        }
        if (elapsed <= HOVER_VISIBLE_MS + HOVER_FADE_MS) {
            return 1.0 - ((elapsed - HOVER_VISIBLE_MS) / (double) HOVER_FADE_MS);
        }
        return 0.0;
    }

    private static void appendZoneFeedback(List<HudRenderCommand> out, MovementState.ZoneKind zoneKind) {
        switch (zoneKind) {
            case DEAD -> out.add(HudRenderCommand.edgeVignette(HudRenderLayer.MOVEMENT_HUD, 0x66000000));
            case NEGATIVE -> out.add(HudRenderCommand.edgeVignette(HudRenderLayer.MOVEMENT_HUD, 0x553A4A7A));
            case RESIDUE_ASH -> out.add(HudRenderCommand.edgeVignette(HudRenderLayer.MOVEMENT_HUD, 0x554B3A2E));
            case NORMAL -> {
            }
        }
    }

    private static void appendCooldown(
        List<HudRenderCommand> out,
        int x,
        int y,
        int width,
        long remainingTicks,
        long maxTicks,
        int color,
        double alpha
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.MOVEMENT_HUD, x, y, width, 3, withAlpha(0x80303A42, alpha)));
        double readyRatio = 1.0 - Math.max(0.0, Math.min(1.0, remainingTicks / (double) maxTicks));
        int fill = Math.max(0, Math.min(width, (int) Math.round(width * readyRatio)));
        if (fill > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.MOVEMENT_HUD, x, y, fill, 3, withAlpha(color, alpha)));
        }
    }

    private static void appendStamina(
        List<HudRenderCommand> out,
        MovementState state,
        int x,
        int y,
        double alpha,
        long nowMs
    ) {
        int width = 34;
        out.add(HudRenderCommand.rect(HudRenderLayer.MOVEMENT_HUD, x, y, width, 3, withAlpha(0x80303A42, alpha)));
        int fill = Math.max(0, Math.min(width, (int) Math.round(width * state.staminaRatio())));
        if (fill <= 0) {
            return;
        }
        boolean recentCost = state.staminaCostActive() && nowMs - state.hudActivityAtMs() <= REJECT_FLASH_MS;
        int color = state.lowStamina() ? STAMINA_LOW_COLOR : (recentCost ? STAMINA_COLOR : 0xFFFFD060);
        out.add(HudRenderCommand.rect(HudRenderLayer.MOVEMENT_HUD, x, y, fill, 3, withAlpha(color, alpha)));
    }

    private static void appendDoubleJumpDots(
        List<HudRenderCommand> out,
        MovementState state,
        int x,
        int y,
        double alpha
    ) {
        int max = Math.max(0, Math.min(2, state.doubleJumpChargesMax()));
        for (int i = 0; i < max; i++) {
            int color = i < state.doubleJumpChargesRemaining() ? DOT_READY_COLOR : DOT_SPENT_COLOR;
            out.add(HudRenderCommand.rect(HudRenderLayer.MOVEMENT_HUD, x + i * 9, y, 6, 6, withAlpha(color, alpha)));
        }
    }

    private static int withAlpha(int argb, double alphaMultiplier) {
        int baseAlpha = (argb >>> 24) & 0xFF;
        int alpha = Math.max(0, Math.min(255, (int) Math.round(baseAlpha * alphaMultiplier)));
        return (alpha << 24) | (argb & 0x00FFFFFF);
    }
}
