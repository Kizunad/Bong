package com.bong.client.hud;

import com.bong.client.combat.UnlockedStyles;
import com.bong.client.combat.store.DerivedAttrsStore;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-HUD-v1 §3.4：替尸伪皮层数与绝灵涡流短角标。
 */
public final class StyleBadgeHudPlanner {
    static final int BADGE_H = 12;
    static final int FAKE_SKIN_W = 34;
    static final int VORTEX_W = 20;
    static final int GAP = 3;
    static final int BG_COLOR = 0xA0101010;
    static final int FAKE_SKIN_COLOR = 0xFFE0C080;
    static final int VORTEX_READY_COLOR = 0xFF60B0FF;

    private StyleBadgeHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        UnlockedStyles unlockedStyles,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        UnlockedStyles unlocked = unlockedStyles == null ? UnlockedStyles.none() : unlockedStyles;
        DerivedAttrsStore.State state = DerivedAttrsStore.snapshot();
        boolean showFakeSkin = unlocked.tishi() && state.tuikeLayers() > 0;
        boolean showVortex = unlocked.jueling() && state.vortexActive();
        if (!showFakeSkin && !showVortex) return out;

        int x = MiniBodyHudPlanner.MARGIN_X;
        int y = screenHeight - MiniBodyHudPlanner.PANEL_H - MiniBodyHudPlanner.MARGIN_Y - BADGE_H - 2;
        if (y < 2) y = 2;

        if (showFakeSkin) {
            int layers = Math.min(9, state.tuikeLayers());
            appendBadge(out, x, y, FAKE_SKIN_W, "伪×" + layers, FAKE_SKIN_COLOR);
            x += FAKE_SKIN_W + GAP;
        }
        if (showVortex) {
            appendBadge(out, x, y, VORTEX_W, "涡", VORTEX_READY_COLOR);
        }

        return out;
    }

    private static void appendBadge(
        List<HudRenderCommand> out,
        int x,
        int y,
        int width,
        String label,
        int accentColor
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, width, BADGE_H, BG_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, width, 1, accentColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y + BADGE_H - 1, width, 1, accentColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, 1, BADGE_H, accentColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x + width - 1, y, 1, BADGE_H, accentColor));
        out.add(HudRenderCommand.text(HudRenderLayer.DERIVED_ATTR, label, x + 3, y + 2, accentColor));
    }
}
