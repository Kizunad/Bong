package com.bong.client.hud;

import com.bong.client.combat.store.DerivedAttrsStore;

import java.util.ArrayList;
import java.util.List;

/**
 * Large DerivedAttr icons (plan §U6–U7 / §1 "DerivedAttrs 大图标"). Shown only
 * when at least one special attr is active. Icons are drawn as simple colored
 * rectangles with an ASCII glyph — placeholder art per plan §0.4.
 */
public final class DerivedAttrIconHudPlanner {
    public static final int ICON_SIZE = 22;
    public static final int ICON_GAP = 4;
    public static final int BOTTOM_MARGIN = 110;
    public static final int LEFT_MARGIN = 12;

    public static final int FLY_COLOR = 0xC0204080;
    public static final int PHASE_COLOR = 0xC0800080;
    public static final int TRIB_LOCK_COLOR = 0xC0800000;
    public static final int BORDER_COLOR = 0xFFFFFFFF;

    private DerivedAttrIconHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) return out;
        DerivedAttrsStore.State state = DerivedAttrsStore.snapshot();
        if (!state.flying() && !state.phasing() && !state.tribulationLocked()) return out;

        int x = LEFT_MARGIN;
        int y = screenHeight - ICON_SIZE - BOTTOM_MARGIN;

        if (state.flying()) {
            drawIcon(out, x, y, FLY_COLOR, "\u98de");
            x += ICON_SIZE + ICON_GAP;
        }
        if (state.phasing()) {
            drawIcon(out, x, y, PHASE_COLOR, "\u865a");
            x += ICON_SIZE + ICON_GAP;
        }
        if (state.tribulationLocked()) {
            drawIcon(out, x, y, TRIB_LOCK_COLOR, "\u52ab");
        }
        return out;
    }

    private static void drawIcon(List<HudRenderCommand> out, int x, int y, int color, String glyph) {
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, ICON_SIZE, ICON_SIZE, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, ICON_SIZE, 1, BORDER_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y + ICON_SIZE - 1, ICON_SIZE, 1, BORDER_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x, y, 1, ICON_SIZE, BORDER_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.DERIVED_ATTR, x + ICON_SIZE - 1, y, 1, ICON_SIZE, BORDER_COLOR));
        out.add(HudRenderCommand.text(HudRenderLayer.DERIVED_ATTR, glyph, x + 7, y + 7, 0xFFFFFFFF));
    }
}
