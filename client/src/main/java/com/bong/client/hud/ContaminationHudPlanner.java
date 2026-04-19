package com.bong.client.hud;

import com.bong.client.alchemy.state.ContaminationWarningStore;

import java.util.ArrayList;
import java.util.List;

/**
 * 丹毒 mini bar — plan-alchemy-v1 §2.1。
 *
 * <p>沉浸式极简原则(`feedback_hud_immersive_minimal`):仅在 current &gt; 0 时显示,
 * 平时完全隐藏。位置贴在 {@link MiniBodyHudPlanner} 右侧(不与 qi/stamina 条冲突)。
 *
 * <p>布局:两条 8×40 竖条(Mellow 棕 / Violent 红),仅 current &gt; 0 时入场;
 * !ok 时用 0xFFFF6060 闪烁边框,与 MiniBody 低值闪烁保持一致风格。
 */
public final class ContaminationHudPlanner {
    static final int BAR_W = 8;
    static final int BAR_H = 40;
    static final int BAR_GAP = 3;
    /** 紧贴 mini-body 区域右侧:MARGIN_X + PANEL_W + 间距 */
    static final int X_OFFSET_FROM_LEFT = MiniBodyHudPlanner.MARGIN_X
        + MiniBodyHudPlanner.PANEL_W + 4;
    static final int Y_OFFSET_FROM_BOTTOM = MiniBodyHudPlanner.MARGIN_Y + 30;

    static final int TRACK_COLOR = 0xCC202020;
    static final int MELLOW_FILL = 0xCCC09040;
    static final int VIOLENT_FILL = 0xCCE05050;
    static final int WARN_BORDER = 0xFFFF6060;

    private ContaminationHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        // 守卫:测试调用以 (0,0) 触发(无 screen 上下文),不渲染。
        if (screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        ContaminationWarningStore.Snapshot c = ContaminationWarningStore.snapshot();
        boolean hasMellow = c.mellowCurrent() > 0.0001f;
        boolean hasViolent = c.violentCurrent() > 0.0001f;
        if (!hasMellow && !hasViolent) {
            return List.of();
        }
        List<HudRenderCommand> out = new ArrayList<>();
        int baseX = X_OFFSET_FROM_LEFT;
        int baseY = screenHeight - Y_OFFSET_FROM_BOTTOM - BAR_H;
        if (hasMellow) {
            addBar(out, baseX, baseY, c.mellowCurrent(), c.mellowMax(), c.mellowOk(), MELLOW_FILL);
            baseX += BAR_W + BAR_GAP;
        }
        if (hasViolent) {
            addBar(out, baseX, baseY, c.violentCurrent(), c.violentMax(), c.violentOk(), VIOLENT_FILL);
        }
        return List.copyOf(out);
    }

    private static void addBar(List<HudRenderCommand> out, int x, int y,
                               float cur, float max, boolean ok, int fillColor) {
        out.add(HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, x, y, BAR_W, BAR_H, TRACK_COLOR));
        float ratio = max > 0 ? Math.max(0f, Math.min(1f, cur / max)) : 0f;
        int fillH = Math.max(1, Math.round(BAR_H * ratio));
        int fillY = y + (BAR_H - fillH);
        out.add(HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, x, fillY, BAR_W, fillH, fillColor));
        if (!ok) {
            // 边框:四条 1px 矩形勾勒
            out.add(HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, x, y, BAR_W, 1, WARN_BORDER));
            out.add(HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, x, y + BAR_H - 1, BAR_W, 1, WARN_BORDER));
            out.add(HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, x, y, 1, BAR_H, WARN_BORDER));
            out.add(HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, x + BAR_W - 1, y, 1, BAR_H, WARN_BORDER));
        }
    }
}
