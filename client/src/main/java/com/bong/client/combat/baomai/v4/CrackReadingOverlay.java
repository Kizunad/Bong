package com.bong.client.combat.baomai.v4;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.font.TextRenderer;

import java.util.List;

/**
 * plan-combat-skill-feedback-bridges-v1 P1 — 裂读 HUD overlay 渲染器。
 *
 * 差异化视觉：经脉 bracket 色阶（Intact=白/MicroTear=黄/Torn=橙/Severed=红），
 * 深读时额外图标（回路=紫环/死装甲=蓝骨），与 resonance_lock meter 完全不同渲染路径。
 *
 * 渲染位置：左上角（x=8, y=8），列表形式，每行 10px 间隔。
 */
public final class CrackReadingOverlay {

    /** bracket 颜色（0xAARRGGBB ARGB，alpha=0xFF 全不透明）。 */
    private static final int COLOR_INTACT    = 0xFFCCCCCC; // 白/灰 — 完好
    private static final int COLOR_MICRO_TEAR = 0xFFFFDD44; // 黄   — 微裂
    private static final int COLOR_TORN       = 0xFFFF8800; // 橙   — 撕裂
    private static final int COLOR_SEVERED    = 0xFFFF3333; // 红   — 断脉

    /** 深读特效：has_circuit=紫，is_dead_armor=蓝。 */
    private static final int COLOR_CIRCUIT    = 0xFFBB44FF; // 紫
    private static final int COLOR_DEAD_ARMOR = 0xFF4488FF; // 蓝

    private static final int OVERLAY_X = 8;
    private static final int OVERLAY_Y = 8;
    private static final int LINE_HEIGHT = 10;

    private CrackReadingOverlay() {}

    /**
     * 在游戏内 HUD layer 调用（每帧一次，GameRenderer 或 InGameHud 里注册的 callback）。
     *
     * @param context  DrawContext（Fabric 1.20.1）
     * @param renderer 字体渲染器
     * @param nowMs    当前毫秒时间戳
     */
    public static void render(DrawContext context, TextRenderer renderer, long nowMs) {
        CrackReadingHudStateStore.State state = CrackReadingHudStateStore.snapshot(nowMs);
        if (!state.isActive() || state.entries.isEmpty()) {
            return;
        }

        int y = OVERLAY_Y;
        // 标题行
        String title = state.isDeep ? "§5裂读（深）" : "§7裂读";
        context.drawText(renderer, title, OVERLAY_X, y, 0xFFFFFFFF, true);
        y += LINE_HEIGHT;

        List<CrackReadingHudStateStore.MeridianEntry> entries = state.entries;
        int shown = Math.min(entries.size(), 20); // cap 以防万一
        for (int i = 0; i < shown; i++) {
            CrackReadingHudStateStore.MeridianEntry entry = entries.get(i);
            int color = bracketColor(entry.bracket());
            StringBuilder sb = new StringBuilder();
            sb.append(entry.meridian()).append(" [").append(entry.bracket()).append("]");
            if (state.isDeep && entry.hasCircuit()) {
                sb.append(" ○"); // 回路标记
            }
            if (state.isDeep && entry.isDeadArmor()) {
                sb.append(" ■"); // 死装甲标记
            }
            context.drawText(renderer, sb.toString(), OVERLAY_X, y, color, true);
            y += LINE_HEIGHT;
        }
    }

    private static int bracketColor(String bracket) {
        return switch (bracket) {
            case "Intact" -> COLOR_INTACT;
            case "MicroTear" -> COLOR_MICRO_TEAR;
            case "Torn" -> COLOR_TORN;
            case "Severed" -> COLOR_SEVERED;
            default -> COLOR_INTACT;
        };
    }
}
