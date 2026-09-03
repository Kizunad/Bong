package com.bong.client.hud;

import com.bong.client.state.PlayerStateViewModel;

/**
 * QI_RADAR 的语义规划器。
 *
 * <p>境界门、布局锚点和负灵压 tint 属于应用层决策，集中在这里后，SVG
 * 表现后端不再依赖 PlayerStateStore 或具体业务门控。</p>
 */
public final class SvgHudFramePlanner {
    private static final int PANEL_SIZE = 58;
    private static final int ANCHOR_X = 10 + 78 + 8;
    private static final int BOTTOM_MARGIN = 10;
    private static final int DEFAULT_TINT = 0xFFFFFFFF;
    private static final int NEGATIVE_QI_TINT = 0xFFB58CE8;

    private SvgHudFramePlanner() {
    }

    public static SvgHudFrame plan(PlayerStateViewModel player, int screenWidth, int screenHeight) {
        PlayerStateViewModel state = player == null ? PlayerStateViewModel.empty() : player;
        if (screenWidth <= 0 || screenHeight <= 0 || !HudRealmGate.atLeastCondense(state.realm())) {
            return SvgHudFrame.hidden();
        }
        int y = screenHeight - PANEL_SIZE - BOTTOM_MARGIN;
        int tint = state.localNegPressure() < 0.0 ? NEGATIVE_QI_TINT : DEFAULT_TINT;
        return new SvgHudFrame(true, ANCHOR_X, y, 1.0f, tint);
    }
}
