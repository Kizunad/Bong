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
    static final int MINI_PANEL_WIDTH = 132;
    static final int MINI_PANEL_HEIGHT = 42;
    static final int MINI_PANEL_BG = 0xCC101416;
    static final int MINI_TRACK_BG = 0xFF1E2528;

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

        renderPlotOverlay(surface, LingtianPlotVisualState.fromSnapshot(snapshot));
    }

    static String formatLabel(LingtianSessionStore.Snapshot s) {
        StringBuilder sb = new StringBuilder();
        sb.append(s.kind().label());
        sb.append("中... ");
        sb.append(s.elapsedTicks()).append('/').append(s.targetTicks());
        if (s.plantId() != null && !s.plantId().isEmpty()) {
            sb.append(" · ").append(s.plantId());
        }
        if (s.dyeContaminationWarning()) {
            sb.append(" · 已染杂");
        }
        return sb.toString();
    }

    static void renderPlotOverlay(HudSurface surface, LingtianPlotVisualState state) {
        if (state == null || state.title().isEmpty()) {
            return;
        }

        int x = surface.windowWidth() / 2 + 14;
        int y = surface.windowHeight() / 2 - 32;
        int panelRight = Math.min(surface.windowWidth() - 4, x + MINI_PANEL_WIDTH);
        x = Math.max(4, panelRight - MINI_PANEL_WIDTH);

        surface.fill(x + 1, y + 1, x + MINI_PANEL_WIDTH + 1, y + MINI_PANEL_HEIGHT + 1, 0x66000000);
        surface.fill(x, y, x + MINI_PANEL_WIDTH, y + MINI_PANEL_HEIGHT, MINI_PANEL_BG);
        surface.fill(x, y, x + MINI_PANEL_WIDTH, y + 1, state.runeColor());
        surface.fill(x, y + MINI_PANEL_HEIGHT - 1, x + MINI_PANEL_WIDTH, y + MINI_PANEL_HEIGHT, state.runeColor());
        surface.fill(x, y, x + 1, y + MINI_PANEL_HEIGHT, state.runeColor());
        surface.fill(x + MINI_PANEL_WIDTH - 1, y, x + MINI_PANEL_WIDTH, y + MINI_PANEL_HEIGHT, state.runeColor());

        surface.fill(x + 5, y + 6, x + 25, y + 26, state.runeColor() & 0x99FFFFFF);
        surface.drawText(state.icon(), x + 10, y + 12, LABEL_COLOR, true);

        String title = clipByMeasure(surface, state.title(), 92);
        String detail = clipByMeasure(surface, state.detail(), 92);
        surface.drawText(title, x + 31, y + 6, LABEL_COLOR, true);
        surface.drawText(detail, x + 31, y + 17, 0xFFB8C8C8, true);

        int trackX = x + 31;
        int trackY = y + 31;
        int trackW = MINI_PANEL_WIDTH - 38;
        surface.fill(trackX, trackY, trackX + trackW, trackY + 4, MINI_TRACK_BG);
        int filled = Math.round(trackW * state.progress());
        if (filled > 0) {
            surface.fill(trackX, trackY, trackX + filled, trackY + 4, state.fillColor());
        }
    }

    private static String clipByMeasure(HudSurface surface, String text, int maxWidth) {
        if (surface.measureText(text) <= maxWidth) {
            return text;
        }
        String ellipsis = "...";
        int limit = Math.max(0, maxWidth - surface.measureText(ellipsis));
        StringBuilder clipped = new StringBuilder();
        for (int i = 0; i < text.length(); i++) {
            String next = clipped.toString() + text.charAt(i);
            if (surface.measureText(next) > limit) {
                break;
            }
            clipped.append(text.charAt(i));
        }
        return clipped + ellipsis;
    }
}
