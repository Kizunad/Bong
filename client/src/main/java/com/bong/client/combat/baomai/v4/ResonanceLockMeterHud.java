package com.bong.client.combat.baomai.v4;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.font.TextRenderer;

/**
 * plan-combat-skill-feedback-bridges-v1 P1 — 共振锁定倒计时弧形 meter HUD。
 *
 * 锁定中显示：右下角倒计时弧形 meter + 对手 ID 标签。
 * 与 crack_reading overlay（左上角经脉列表）完全不同的渲染位置和视觉风格。
 *
 * 差异化视觉：
 *   - 位置：右下角（screenW - 60, screenH - 60）
 *   - 颜色：深紫主色 0xFF7B2FBE（激活）→ 浅紫 0xFFD4AAFF（临近到期闪烁）
 *   - 进度弧：角度 0°（开始）→ 360°（满）表示锁定时间消耗百分比
 *   - 闪烁：最后 10 tick 时弧形开始闪烁（强调危险）
 */
public final class ResonanceLockMeterHud {

    private static final int COLOR_ACTIVE    = 0xFF7B2FBE; // 深紫
    private static final int COLOR_CRITICAL  = 0xFFD4AAFF; // 浅紫（最后 10 tick）
    private static final long BLINK_THRESHOLD_TICKS = 10L;
    private static final int METER_RADIUS = 20;
    private static final int METER_X_OFFSET = 60;
    private static final int METER_Y_OFFSET = 60;

    private ResonanceLockMeterHud() {}

    /**
     * 每帧调用（GameRenderer 或 InGameHud callback）。
     *
     * @param context     DrawContext
     * @param renderer    TextRenderer
     * @param screenW     屏幕宽度（px）
     * @param screenH     屏幕高度（px）
     * @param currentTick 当前估算 tick（客户端本地）
     * @param nowMs       System.currentTimeMillis()
     */
    public static void render(DrawContext context, TextRenderer renderer,
                              int screenW, int screenH,
                              long currentTick, long nowMs) {
        ResonanceLockHudStateStore.State state = ResonanceLockHudStateStore.snapshot();
        if (!state.locked) return;

        long remaining = state.remainingTicks(currentTick);
        double progress = state.progress(currentTick);

        int cx = screenW - METER_X_OFFSET;
        int cy = screenH - METER_Y_OFFSET;

        boolean isCritical = remaining <= BLINK_THRESHOLD_TICKS;
        boolean blinkVisible = !isCritical || ((nowMs / 250) % 2 == 0); // 250ms 间隔闪
        if (!blinkVisible) return;

        int color = isCritical ? COLOR_CRITICAL : COLOR_ACTIVE;

        // 弧形近似：用线段围成的多边形（每 6° 一段，共 60 段），仅绘 progress 比例部分
        int segments = (int) Math.round(progress * 60);
        for (int i = 0; i < segments; i++) {
            double angle1 = Math.toRadians(-90.0 + i * 6.0);
            double angle2 = Math.toRadians(-90.0 + (i + 1) * 6.0);
            int x1 = (int) (cx + METER_RADIUS * Math.cos(angle1));
            int y1 = (int) (cy + METER_RADIUS * Math.sin(angle1));
            int x2 = (int) (cx + (METER_RADIUS - 3) * Math.cos(angle1));
            int y2 = (int) (cy + (METER_RADIUS - 3) * Math.sin(angle1));
            // 绘制小矩形块（厚度 2px）
            context.fill(Math.min(x1, x2), Math.min(y1, y2),
                         Math.max(x1, x2) + 2, Math.max(y1, y2) + 2, color);
        }

        // 标签：对手名 + 剩余 tick
        String partnerShort = shortName(state.partnerId);
        String label = partnerShort + " " + remaining + "t";
        context.drawText(renderer, label, cx - renderer.getWidth(label) / 2, cy + METER_RADIUS + 4, color, true);

        // 中心标记
        context.fill(cx - 2, cy - 2, cx + 2, cy + 2, color);
    }

    private static String shortName(String id) {
        if (id == null || id.isEmpty()) return "?";
        String stripped = id.startsWith("offline:") ? id.substring(8) : id;
        return stripped.length() > 8 ? stripped.substring(0, 8) + "…" : stripped;
    }
}
