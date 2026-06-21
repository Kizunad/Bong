package com.bong.client.hud;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 震脉（zhenmai）5 招 + 盾格挡的专属条件式 HUD planner。
 *
 * <p>严格遵守 Bong HUD 极简哲学：每个维度只在 {@link ZhenmaiHudStateStore.Slot#active(long)}
 * 为真时 emit，未激活/不相关时完全隐藏。优先瞬态闪现 / 微条 / 中心环高亮，绝不新增常驻面板。
 *
 * <p>呈现策略（与各招的粒子/音效/动画对齐配色）：
 * <ul>
 *   <li>parry（极限弹反）：中心十字环瞬态增亮（血红 {@code #B6172F}），呼应 jiemai_burst_blood 粒子。</li>
 *   <li>neutralize（局部中和）：屏幕中下方瞬态去毒提示「中和 N%」（灰白 {@code #9CA3AF}）。</li>
 *   <li>multipoint（多点反震）：中心右侧微指示「反震 ×N」（暗红 {@code #9B1C31}），仅激活期。</li>
 *   <li>harden（护脉）：左侧护脉微条 + 剩余减伤%（金 {@code #C7A94B}），仅 buff 期。</li>
 *   <li>sever_chain（绝脉断链）：左侧断脉标记「✕脉名」+ 增幅倒计时微条（金 {@code #F4C542}）。</li>
 *   <li>shield_block（盾格挡）：准星下方瞬态盾弧确认（青蓝 {@code #4DA6FF}）。</li>
 * </ul>
 */
public final class ZhenmaiHudPlanner {

    // ── 配色（与服务端 emit_skill_feedback 的 particle color 对齐）──
    static final int PARRY_COLOR = 0xFFB6172F;
    static final int NEUTRALIZE_COLOR = 0xFFCBD3DB;
    static final int MULTIPOINT_COLOR = 0xFFD9405A;
    static final int HARDEN_COLOR = 0xFFC7A94B;
    static final int SEVER_COLOR = 0xFFF4C542;
    static final int SHIELD_COLOR = 0xFF4DA6FF;
    static final int BAR_BG_COLOR = 0x88101018;

    // ── 几何 ──
    static final int PARRY_RING_RADIUS = 26;
    static final int PARRY_RING_THICKNESS = 3;
    static final int BAR_WIDTH = 64;
    static final int BAR_HEIGHT = 4;

    private ZhenmaiHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        ZhenmaiHudStateStore.Snapshot snapshot,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (snapshot == null || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        int cx = screenWidth / 2;
        int cy = screenHeight / 2;

        if (snapshot.parry().active(nowMillis)) {
            appendParry(out, cx, cy);
        }
        if (snapshot.neutralize().active(nowMillis)) {
            appendNeutralize(out, snapshot.neutralize(), cx, cy);
        }
        if (snapshot.multipoint().active(nowMillis)) {
            appendMultipoint(out, snapshot.multipoint(), cx, cy);
        }
        if (snapshot.harden().active(nowMillis)) {
            appendHarden(out, snapshot.harden(), screenHeight);
        }
        if (snapshot.sever().active(nowMillis)) {
            appendSever(out, snapshot.sever(), nowMillis, screenHeight);
        }
        if (snapshot.shield().active(nowMillis)) {
            appendShield(out, cx, cy);
        }
        return out;
    }

    /** 极限弹反成功：中心十字环瞬态增亮（4 段构成菱形环，复用 JiemaiRing 视觉语汇）。 */
    private static void appendParry(List<HudRenderCommand> out, int cx, int cy) {
        int r = PARRY_RING_RADIUS;
        int t = PARRY_RING_THICKNESS;
        out.add(HudRenderCommand.rect(HudRenderLayer.ZHENMAI_PARRY, cx - r, cy - r, r * 2, t, PARRY_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.ZHENMAI_PARRY, cx - r, cy + r - t, r * 2, t, PARRY_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.ZHENMAI_PARRY, cx - r, cy - r, t, r * 2, PARRY_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.ZHENMAI_PARRY, cx + r - t, cy - r, t, r * 2, PARRY_COLOR));
    }

    /** 局部中和：中下方瞬态去毒提示。 */
    private static void appendNeutralize(List<HudRenderCommand> out, ZhenmaiHudStateStore.Slot slot, int cx, int cy) {
        String meridian = slot.label().isEmpty() ? "" : slot.label() + " ";
        String text = String.format(Locale.ROOT, "%s中和 -%.1f%%", meridian, slot.value1());
        out.add(HudRenderCommand.text(HudRenderLayer.ZHENMAI_NEUTRALIZE, text, cx - 36, cy + 32, NEUTRALIZE_COLOR));
    }

    /** 多点反震：中心右上微指示「反震 ×N」。 */
    private static void appendMultipoint(List<HudRenderCommand> out, ZhenmaiHudStateStore.Slot slot, int cx, int cy) {
        String text = "反震 ×" + Math.max(0, slot.intValue());
        out.add(HudRenderCommand.rect(HudRenderLayer.ZHENMAI_MULTIPOINT, cx + 22, cy - 24, 50, 12, BAR_BG_COLOR));
        out.add(HudRenderCommand.text(HudRenderLayer.ZHENMAI_MULTIPOINT, text, cx + 26, cy - 22, MULTIPOINT_COLOR));
    }

    /** 护脉：左侧 buff 微条 + 减伤%。 */
    private static void appendHarden(List<HudRenderCommand> out, ZhenmaiHudStateStore.Slot slot, int screenHeight) {
        int x = 12;
        int y = Math.max(24, screenHeight / 2 - 4);
        // 减伤比例越高，填充越满（reduction 是「伤害降低」，1.0 = 全免）。
        int filled = Math.max(1, Math.round(BAR_WIDTH * slot.value1()));
        out.add(HudRenderCommand.rect(HudRenderLayer.ZHENMAI_HARDEN, x, y, BAR_WIDTH, BAR_HEIGHT, BAR_BG_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.ZHENMAI_HARDEN, x, y, filled, BAR_HEIGHT, HARDEN_COLOR));
        String meridian = slot.label().isEmpty() ? "" : slot.label() + " ";
        String text = String.format(Locale.ROOT, "护脉 %s减伤%.0f%%", meridian, slot.value1() * 100f);
        out.add(HudRenderCommand.text(HudRenderLayer.ZHENMAI_HARDEN, text, x, y - 10, HARDEN_COLOR));
    }

    /** 绝脉断链：左侧断脉标记 + 增幅倒计时微条。 */
    private static void appendSever(List<HudRenderCommand> out, ZhenmaiHudStateStore.Slot slot, long nowMillis, int screenHeight) {
        int x = 12;
        int y = Math.max(40, screenHeight / 2 + 14);
        String meridian = slot.label().isEmpty() ? "脉" : slot.label();
        out.add(HudRenderCommand.text(HudRenderLayer.ZHENMAI_SEVER, "✕" + meridian + " 断链增幅", x, y - 10, SEVER_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.ZHENMAI_SEVER, x, y, BAR_WIDTH, BAR_HEIGHT, BAR_BG_COLOR));
        // 增幅窗口剩余比例：用 slot expiry 推 — 调用方传入的 durationMillis 决定窗口长度，
        // 这里以「expiry - now」做剩余条（不假设固定总长，剩余越多越满）。
        long remaining = Math.max(0L, slot.expiresAtMillis() - nowMillis);
        // 60s 增幅窗口为参考刻度（与 server BACKFIRE_AMPLIFICATION_TICKS=60s 对齐）。
        float ratio = Math.min(1f, remaining / 60_000f);
        int filled = Math.max(1, Math.round(BAR_WIDTH * ratio));
        out.add(HudRenderCommand.rect(HudRenderLayer.ZHENMAI_SEVER, x, y, filled, BAR_HEIGHT, SEVER_COLOR));
    }

    /** 盾格挡命中：准星下方瞬态盾弧确认（短横 + 两侧斜角近似弧）。 */
    private static void appendShield(List<HudRenderCommand> out, int cx, int cy) {
        int y = cy + 14;
        // 中段横条
        out.add(HudRenderCommand.rect(HudRenderLayer.SHIELD_BLOCK, cx - 10, y, 20, 2, SHIELD_COLOR));
        // 两侧上挑的小段，近似盾形弧
        out.add(HudRenderCommand.rect(HudRenderLayer.SHIELD_BLOCK, cx - 12, y - 2, 2, 4, SHIELD_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.SHIELD_BLOCK, cx + 10, y - 2, 2, 4, SHIELD_COLOR));
    }
}
