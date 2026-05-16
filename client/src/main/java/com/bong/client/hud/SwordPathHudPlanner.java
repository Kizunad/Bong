package com.bong.client.hud;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class SwordPathHudPlanner {
    private static final int GRADE_ICON_SIZE = 16;
    private static final int QI_BAR_WIDTH = 3;
    private static final int QI_BAR_HEIGHT = 24;
    private static final int BOND_ARC_RADIUS = 8;

    private static final int BG_COLOR = 0xB0121118;
    private static final int QI_BAR_BG = 0xFF1A1A2A;
    private static final int HEAVEN_PULSE_LO = 0x78FFFFFF;
    private static final int HEAVEN_PULSE_HI = 0xDCFFFFFF;

    private static final int[] GRADE_COLORS = {
        0xFF888888,
        0xFF99AABB,
        0xFFAABBCC,
        0xFFBBCCDD,
        0xFFCCDDEE,
        0xFFDDEEFF,
        0xFFEEF4FF,
    };

    private SwordPathHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        SwordBondHudState state,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active() || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        int baseX = screenWidth - 36;
        int baseY = screenHeight - 80;

        appendGradeIcon(out, baseX, baseY, state.grade(), state.gradeName());
        appendStoredQiBar(out, baseX + GRADE_ICON_SIZE + 2, baseY, state.storedQiRatio(), state.grade());
        appendBondArc(out, baseX + GRADE_ICON_SIZE / 2, baseY + GRADE_ICON_SIZE + 2, state.bondStrength());

        if (state.heavenGateReady()) {
            appendHeavenGateReady(out, baseX, baseY - 14, nowMillis);
        }

        return out;
    }

    private static void appendGradeIcon(List<HudRenderCommand> out, int x, int y, int grade, String name) {
        int color = gradeColor(grade);
        out.add(HudRenderCommand.rect(HudRenderLayer.SWORD_BOND, x, y, GRADE_ICON_SIZE, GRADE_ICON_SIZE, BG_COLOR));
        String label = String.valueOf(grade);
        out.add(HudRenderCommand.text(HudRenderLayer.SWORD_BOND, label, x + 5, y + 4, color));
    }

    private static void appendStoredQiBar(List<HudRenderCommand> out, int x, int y, float ratio, int grade) {
        int color = gradeColor(grade);
        out.add(HudRenderCommand.rect(HudRenderLayer.SWORD_BOND, x, y, QI_BAR_WIDTH, QI_BAR_HEIGHT, QI_BAR_BG));
        int fillH = Math.round(QI_BAR_HEIGHT * ratio);
        if (fillH > 0) {
            out.add(HudRenderCommand.rect(
                HudRenderLayer.SWORD_BOND,
                x, y + QI_BAR_HEIGHT - fillH,
                QI_BAR_WIDTH, fillH,
                color
            ));
        }
    }

    private static void appendBondArc(List<HudRenderCommand> out, int cx, int cy, float strength) {
        int alpha = Math.round(200 * strength);
        int color = (alpha << 24) | 0x667788;
        int arcW = Math.round(BOND_ARC_RADIUS * 2 * strength);
        if (arcW > 0) {
            out.add(HudRenderCommand.rect(
                HudRenderLayer.SWORD_BOND,
                cx - arcW / 2, cy,
                arcW, 2,
                color
            ));
        }
    }

    private static void appendHeavenGateReady(List<HudRenderCommand> out, int x, int y, long nowMillis) {
        int pulse = (int) ((nowMillis / 25) % 40);
        boolean bright = pulse < 20;
        int color = bright ? HEAVEN_PULSE_HI : HEAVEN_PULSE_LO;
        out.add(HudRenderCommand.text(HudRenderLayer.SWORD_BOND, "天门可开", x - 8, y, color));
    }

    static int gradeColor(int grade) {
        if (grade < 0) return GRADE_COLORS[0];
        if (grade >= GRADE_COLORS.length) return GRADE_COLORS[GRADE_COLORS.length - 1];
        return GRADE_COLORS[grade];
    }
}
