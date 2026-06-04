package com.bong.client.hud;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 卧棺 HUD 面板渲染计划（plan-coffin-tiers-v1 P3）。
 *
 * <p>P3 新增档位徽章：面板右侧倍率文字前追加 [凡木/寒玉/玄石/青铜] 标签，
 * 对应寿元倍率 ×0.9 / ×0.7 / ×0.5 / ×0.3。grade=null 时降级显示 [凡木]（默认）。</p>
 */
public final class CoffinHudPlanner {
    public static final String LABEL = "卧棺 · 寿火徐燃";
    public static final int PANEL_WIDTH = 148;
    public static final int PANEL_HEIGHT = 24;
    public static final int BOTTOM_MARGIN = 118;

    private static final int PANEL_COLOR = 0x8A101010;
    private static final int LABEL_COLOR = 0xB8D0D0D0;
    private static final int MULTIPLIER_COLOR = 0xA8B8C8D8;
    /** 档位徽章用淡金色，让其与倍率数字区分。 */
    private static final int GRADE_BADGE_COLOR = 0xFFD4B060;

    private CoffinHudPlanner() {
    }

    /**
     * 将档级字符串（"mundane"/"jade"/"stone"/"bronze"）转换为玩家可读汉字徽章。
     * null 或未知值 → "凡木"（防御性默认，与 Mundane 对齐）。
     */
    public static String gradeBadge(String grade) {
        if (grade == null) {
            return "凡木";
        }
        return switch (grade) {
            case "mundane" -> "凡木";
            case "jade"    -> "寒玉";
            case "stone"   -> "玄石";
            case "bronze"  -> "青铜";
            default        -> "凡木";
        };
    }

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        return buildCommands(CoffinStateStore.snapshot(), screenWidth, screenHeight);
    }

    static List<HudRenderCommand> buildCommands(
        CoffinStateStore.State state,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.inCoffin() || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        int x = (screenWidth - PANEL_WIDTH) / 2;
        int y = screenHeight - BOTTOM_MARGIN;
        out.add(HudRenderCommand.rect(
            HudRenderLayer.COFFIN,
            x,
            y,
            PANEL_WIDTH,
            PANEL_HEIGHT,
            PANEL_COLOR
        ));
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.COFFIN,
            LABEL,
            x + 9,
            y + 5,
            LABEL_COLOR,
            0.75
        ));
        // 档位徽章（plan-coffin-tiers-v1 P3）：紧接 LABEL 后，倍率文字前。
        String badge = "[" + gradeBadge(state.grade()) + "]";
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.COFFIN,
            badge,
            x + 9,
            y + 14,
            GRADE_BADGE_COLOR,
            0.75
        ));
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.COFFIN,
            String.format(Locale.ROOT, "×%.1f", state.lifespanRateMultiplier()),
            x + PANEL_WIDTH - 36,
            y + 5,
            MULTIPLIER_COLOR,
            0.75
        ));
        return out;
    }
}
