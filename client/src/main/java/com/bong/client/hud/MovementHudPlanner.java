package com.bong.client.hud;

import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;

import java.util.ArrayList;
import java.util.List;

public final class MovementHudPlanner {
    public static final long HOVER_VISIBLE_MS = 3_000L;
    public static final long HOVER_FADE_MS = 500L;
    public static final long REJECT_FLASH_MS = 300L;
    public static final int PANEL_WIDTH = 60;
    public static final int PANEL_HEIGHT = 14;

    private static final long DASH_COOLDOWN_MAX_TICKS = 40L;
    private static final int HOTBAR_GAP = 6;
    private static final int EDGE_MARGIN = 4;
    private static final int TRACK_COLOR = 0xA0182228;
    private static final int DASH_COLOR = 0xFF9FD3FF;
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

        PanelGeometry geometry = panelGeometry(screenWidth, screenHeight);
        if (geometry == null) {
            return out;
        }
        int x = geometry.x();
        int y = geometry.y();
        out.add(HudRenderCommand.rect(
            HudRenderLayer.MOVEMENT_HUD,
            x,
            y,
            PANEL_WIDTH,
            PANEL_HEIGHT,
            withAlpha(TRACK_COLOR, alpha)
        ));
        out.add(HudRenderCommand.scaledText(HudRenderLayer.MOVEMENT_HUD, "DASH", x + 6, y + 1, withAlpha(DASH_COLOR, alpha), 0.6));
        appendCooldown(out, x + 6, y + 10, 48, state.dashCooldownRemainingTicks(), DASH_COOLDOWN_MAX_TICKS, DASH_COLOR, alpha);

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
        return timedAlpha(state, nowMs);
    }

    private static PanelGeometry panelGeometry(int screenWidth, int screenHeight) {
        if (screenWidth < PANEL_WIDTH + EDGE_MARGIN * 2 || screenHeight < PANEL_HEIGHT + EDGE_MARGIN * 2) {
            return null;
        }
        int hotbarWidth = QuickBarHudPlanner.TOTAL_SLOTS * QuickBarHudPlanner.SLOT_SIZE
            + (QuickBarHudPlanner.TOTAL_SLOTS - 1) * QuickBarHudPlanner.SLOT_GAP;
        int hotbarLeftX = (screenWidth - hotbarWidth) / 2;
        int hotbarRightX = hotbarLeftX + hotbarWidth;
        int lowerY = screenHeight - QuickBarHudPlanner.LOWER_BOTTOM_MARGIN - QuickBarHudPlanner.SLOT_SIZE;
        int upperY = lowerY - QuickBarHudPlanner.SLOT_SIZE - QuickBarHudPlanner.UPPER_GAP;
        int hotbarTotalHeight = lowerY + QuickBarHudPlanner.SLOT_SIZE - upperY;
        int besideY = upperY + (hotbarTotalHeight - PANEL_HEIGHT) / 2;

        int reservedSideGap = WeaponHotbarHudPlanner.SLOT_GAP_TO_HOTBAR
            + WeaponHotbarHudPlanner.SLOT_W
            + HOTBAR_GAP;
        int rightReservedX = hotbarRightX + reservedSideGap;
        if (rightReservedX + PANEL_WIDTH <= screenWidth - EDGE_MARGIN) {
            return clampPanelGeometry(rightReservedX, besideY, screenWidth, screenHeight);
        }
        int leftReservedX = hotbarLeftX - reservedSideGap - PANEL_WIDTH;
        if (leftReservedX >= EDGE_MARGIN) {
            return clampPanelGeometry(leftReservedX, besideY, screenWidth, screenHeight);
        }
        int leftX = hotbarLeftX - HOTBAR_GAP - PANEL_WIDTH;
        if (leftX >= EDGE_MARGIN) {
            return clampPanelGeometry(leftX, besideY, screenWidth, screenHeight);
        }
        int rightX = hotbarRightX + HOTBAR_GAP;
        if (rightX + PANEL_WIDTH <= screenWidth - EDGE_MARGIN) {
            return clampPanelGeometry(rightX, besideY, screenWidth, screenHeight);
        }

        int aboveX = Math.max(EDGE_MARGIN, Math.min(screenWidth - PANEL_WIDTH - EDGE_MARGIN, (screenWidth - PANEL_WIDTH) / 2));
        int aboveY = Math.max(EDGE_MARGIN, upperY - HOTBAR_GAP - PANEL_HEIGHT);
        return clampPanelGeometry(aboveX, aboveY, screenWidth, screenHeight);
    }

    private static PanelGeometry clampPanelGeometry(int x, int y, int screenWidth, int screenHeight) {
        int clampedX = Math.max(EDGE_MARGIN, Math.min(screenWidth - PANEL_WIDTH - EDGE_MARGIN, x));
        int clampedY = Math.max(EDGE_MARGIN, Math.min(screenHeight - PANEL_HEIGHT - EDGE_MARGIN, y));
        return new PanelGeometry(clampedX, clampedY);
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

    private static int withAlpha(int argb, double alphaMultiplier) {
        int baseAlpha = (argb >>> 24) & 0xFF;
        int alpha = Math.max(0, Math.min(255, (int) Math.round(baseAlpha * alphaMultiplier)));
        return (alpha << 24) | (argb & 0x00FFFFFF);
    }

    private record PanelGeometry(int x, int y) {}
}
