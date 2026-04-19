package com.bong.client.lingtian;

import com.bong.client.BongHud.HudSurface;
import com.bong.client.lingtian.state.LingtianSessionStore;

import java.util.Objects;

/**
 * plan-lingtian-v1 §4 / UI 切片 — 屏幕中下方一条进度条 + label。
 *
 * <p>当 {@link LingtianSessionStore.Snapshot#active} 为 false 时不渲染。
 * 设计参考采集浮窗（{@code docs/svg/harvest-popup.svg}）的简化进度条范式：
 * 中下黑底条 + 浅色填充 + 中央 label "{动作}中... 12/40"。</p>
 *
 * <p>遵循"沉浸式极简"约束（plan-HUD-v1）：仅在 active session 时显示，
 * 不进事件流，会话结束后立即消失。</p>
 */
public final class LingtianSessionHud {

    static final int BAR_WIDTH = 180;
    static final int BAR_HEIGHT = 12;
    static final int BAR_BG_COLOR = 0xCC000000;
    static final int BAR_FILL_COLOR = 0xFF66CC66;
    static final int BAR_BORDER_COLOR = 0xFFB0B0B0;
    static final int LABEL_COLOR = 0xFFF4F4F4;
    /** 距底部像素（让出 hotbar 留出位置）。 */
    static final int BOTTOM_OFFSET = 60;

    private LingtianSessionHud() {}

    public static void render(HudSurface surface, LingtianSessionStore.Snapshot snapshot) {
        Objects.requireNonNull(surface, "surface");
        if (snapshot == null || !snapshot.active()) {
            return;
        }

        int x = (surface.windowWidth() - BAR_WIDTH) / 2;
        int y = surface.windowHeight() - BOTTOM_OFFSET;
        int filledWidth = Math.round(BAR_WIDTH * snapshot.progress());

        // 边框（外 1px）
        surface.fill(x - 1, y - 1, x + BAR_WIDTH + 1, y + BAR_HEIGHT + 1, BAR_BORDER_COLOR);
        // 背景
        surface.fill(x, y, x + BAR_WIDTH, y + BAR_HEIGHT, BAR_BG_COLOR);
        // 填充
        if (filledWidth > 0) {
            surface.fill(x, y, x + filledWidth, y + BAR_HEIGHT, BAR_FILL_COLOR);
        }

        // label
        String text = formatLabel(snapshot);
        int labelWidth = surface.measureText(text);
        int labelX = (surface.windowWidth() - labelWidth) / 2;
        int labelY = y - 12;
        surface.drawText(text, labelX, labelY, LABEL_COLOR, true);
    }

    static String formatLabel(LingtianSessionStore.Snapshot s) {
        StringBuilder sb = new StringBuilder();
        sb.append(s.kind().label());
        sb.append("中... ");
        sb.append(s.elapsedTicks()).append('/').append(s.targetTicks());
        if (s.plantId() != null && !s.plantId().isEmpty()) {
            sb.append(" · ").append(s.plantId());
        }
        return sb.toString();
    }
}
