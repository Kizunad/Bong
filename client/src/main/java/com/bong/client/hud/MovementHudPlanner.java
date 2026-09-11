package com.bong.client.hud;

import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;
import com.bong.client.movement.DashSkill;

import java.util.ArrayList;
import java.util.List;

public final class MovementHudPlanner {
    public static final long REJECT_FLASH_MS = 300L;
    public static final int PANEL_WIDTH = 40;
    public static final int PANEL_HEIGHT = 44;
    public static final String ICON = "bong-client:textures/hud/dash/sidestep.png";

    private static final int HOTBAR_GAP = 6;
    private static final int EDGE_MARGIN = 4;
    private static final int DASH_COLOR = 0xFFE8EDD8;
    private static final int READY_COLOR = 0xFFAFD6BA;
    private static final int REJECT_COLOR = 0xFFE68675;

    private MovementHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight, long nowMs) {
        return buildCommands(MovementStateStore.snapshot(), DashSkill.learned(), screenWidth, screenHeight, nowMs);
    }

    static List<HudRenderCommand> buildCommands(
        MovementState state,
        boolean learned,
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        appendZoneFeedback(out, state.zoneKind());

        if (!learned) return out;

        PanelGeometry geometry = panelGeometry(screenWidth, screenHeight);
        if (geometry == null) {
            return out;
        }
        int x = geometry.x();
        int y = geometry.y();
        double progress = readyFraction(state, nowMs);
        boolean ready = state.dashCooldownRemainingTicks() == 0;
        boolean rejected = state.rejectedRecently(nowMs, REJECT_FLASH_MS);
        long elapsed = Math.max(0, nowMs - state.hudActivityAtMs());
        double flash = state.hudActivityAtMs() > 0 ? Math.max(0, 1 - elapsed / 420.0) : 0;
        int color = rejected ? REJECT_COLOR : DASH_COLOR;

        // 印记保持比例；残影仅在真实动作启动时偏移，冷却完成单独点亮脚下刻痕。
        if (state.action() == MovementState.Action.DASHING && flash > 0) {
            out.add(HudRenderCommand.texture(HudRenderLayer.MOVEMENT_HUD, ICON,
                x, y + 3, 32, 32, withAlpha(READY_COLOR, flash * .4)));
        }
        out.add(HudRenderCommand.svg(HudRenderLayer.MOVEMENT_HUD, "track", x, y,
            PANEL_WIDTH, PANEL_HEIGHT, withAlpha(rejected ? REJECT_COLOR : READY_COLOR, ready ? .65 : .3)));
        out.add(HudRenderCommand.texture(HudRenderLayer.MOVEMENT_HUD, ICON,
            x + 4, y + 2, 32, 32, withAlpha(color, ready ? .85 + .15 * flash : .3 + .35 * progress)));
        for (int i = 0; i < 8; i++) {
            double filled = Math.max(0, Math.min(1, progress * 8 - i));
            out.add(HudRenderCommand.svg(HudRenderLayer.MOVEMENT_HUD, "tick",
                x + 4 + i * 4, y + 35, 4, 5,
                withAlpha(rejected ? REJECT_COLOR : READY_COLOR, .12 + .78 * filled)));
        }
        if ((ready || rejected) && flash > 0) {
            out.add(HudRenderCommand.svg(HudRenderLayer.MOVEMENT_HUD, "flare", x, y,
                PANEL_WIDTH, PANEL_HEIGHT, withAlpha(rejected ? REJECT_COLOR : DASH_COLOR, flash)));
        }
        return out;
    }

    static double readyFraction(MovementState state, long nowMs) {
        if (state.dashCooldownRemainingTicks() == 0) return 1;
        if (state.dashCooldownTotalTicks() == 0) return 0;
        double remaining = state.dashCooldownRemainingTicks()
            - Math.max(0, nowMs - state.receivedAtMs()) / 50.0;
        // 可插值进度，但可用状态必须等服务端确认。
        return Math.max(0, Math.min(.99, 1 - remaining / state.dashCooldownTotalTicks()));
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

    private static void appendZoneFeedback(List<HudRenderCommand> out, MovementState.ZoneKind zoneKind) {
        switch (zoneKind) {
            case DEAD -> out.add(HudRenderCommand.edgeVignette(HudRenderLayer.MOVEMENT_HUD, 0x66000000));
            case NEGATIVE -> out.add(HudRenderCommand.edgeVignette(HudRenderLayer.MOVEMENT_HUD, 0x553A4A7A));
            case RESIDUE_ASH -> out.add(HudRenderCommand.edgeVignette(HudRenderLayer.MOVEMENT_HUD, 0x554B3A2E));
            case NORMAL -> {
            }
        }
    }

    private static int withAlpha(int argb, double alphaMultiplier) {
        int baseAlpha = (argb >>> 24) & 0xFF;
        int alpha = Math.max(0, Math.min(255, (int) Math.round(baseAlpha * alphaMultiplier)));
        return (alpha << 24) | (argb & 0x00FFFFFF);
    }

    private record PanelGeometry(int x, int y) {}
}
