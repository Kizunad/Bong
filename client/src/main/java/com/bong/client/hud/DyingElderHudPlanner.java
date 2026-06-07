package com.bong.client.hud;

import com.bong.client.dying_elder.DyingElderEncounterStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * plan-dying-elder-v1 P3 — 垂死大能遭遇 HUD 渲染计划器。
 *
 * <p>从 {@link DyingElderEncounterStore} 读取状态，生成 {@link HudRenderCommand} 列表：
 * <ul>
 *   <li>真元回复进度条（qi_fraction）</li>
 *   <li>三个交互按钮：给丹 [G] / 拒绝 [H] / 拖延 [J]</li>
 *   <li>背叛概率（SpiritEye 激活时显示具体数字，否则显示"气息有异"）</li>
 * </ul>
 *
 * <p><b>守恒红线</b>：本 Planner 只读取 Store，<strong>绝不</strong>修改任何 gameplay 状态。
 * 按钮键位仅为视觉提示，实际意图通过服务端 IntentHandler 处理。
 */
public final class DyingElderHudPlanner {

    // ── 布局常量 ─────────────────────────────────────────────────────────────

    /** 面板总宽度（px）。 */
    public static final int PANEL_WIDTH = 180;

    /** 面板总高度（px）。 */
    public static final int PANEL_HEIGHT = 60;

    /** 面板距屏幕底部边距（px）。 */
    public static final int BOTTOM_MARGIN = 80;

    // ── 颜色常量 ─────────────────────────────────────────────────────────────

    /** 面板背景：深色半透明。 */
    public static final int COLOR_PANEL_BG = 0x8A101010;

    /** 标题文字颜色：偏蓝白，呼应"化虚大能"气质。 */
    public static final int COLOR_TITLE = 0xFFB0C8E0;

    /** 真元进度条背景色。 */
    public static final int COLOR_QI_BAR_BG = 0x60203040;

    /** 真元进度条前景色：蓝绿色。 */
    public static final int COLOR_QI_BAR_FG = 0xFF40A0C0;

    /** 按钮背景色。 */
    public static final int COLOR_BTN_BG = 0x60303030;

    /** 按钮文字颜色。 */
    public static final int COLOR_BTN_TEXT = 0xFFD0D0D0;

    /** 背叛概率（已知，SpiritEye 激活）高风险色（>= 0.6）。 */
    public static final int COLOR_BETRAY_HIGH = 0xFFE05040;

    /** 背叛概率（已知，SpiritEye 激活）低风险色（< 0.6）。 */
    public static final int COLOR_BETRAY_LOW = 0xFFA0C080;

    /** "气息有异"提示（SpiritEye 未激活）颜色：暗黄色暗示异常。 */
    public static final int COLOR_BETRAY_UNKNOWN = 0xFFB0A040;

    // ── 进度条尺寸 ───────────────────────────────────────────────────────────

    /** 真元进度条高度（px）。 */
    public static final int QI_BAR_HEIGHT = 4;

    /** 真元进度条内边距（px，距面板两侧）。 */
    public static final int QI_BAR_PADDING = 8;

    /** 按钮高度（px）。 */
    public static final int BTN_HEIGHT = 14;

    /** 按钮宽度（px）。 */
    public static final int BTN_WIDTH = 50;

    /** 按钮间距（px）。 */
    public static final int BTN_SPACING = 6;

    private DyingElderHudPlanner() {}

    /**
     * 从当前 Store 状态构建 HUD 渲染指令列表。
     *
     * @param screenWidth  当前屏幕宽度
     * @param screenHeight 当前屏幕高度
     * @return 渲染指令列表（空 = 不显示 HUD）
     */
    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        return buildCommands(
            DyingElderEncounterStore.isActive(),
            DyingElderEncounterStore.getQiFraction(),
            DyingElderEncounterStore.isSpiritEyeActive(),
            DyingElderEncounterStore.getBetrayProbability(),
            DyingElderEncounterStore.getZoneDisplayName(),
            screenWidth,
            screenHeight
        );
    }

    /**
     * 纯函数版本（供测试注入状态）。
     */
    public static List<HudRenderCommand> buildCommands(
        boolean active,
        float qiFraction,
        boolean spiritEyeActive,
        Double betrayProbability,
        String zoneDisplayName,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (!active || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        int panelX = (screenWidth - PANEL_WIDTH) / 2;
        int panelY = screenHeight - BOTTOM_MARGIN - PANEL_HEIGHT;

        // ── 背景面板 ──────────────────────────────────────────────────────────
        out.add(HudRenderCommand.rect(
            HudRenderLayer.DYING_ELDER,
            panelX,
            panelY,
            PANEL_WIDTH,
            PANEL_HEIGHT,
            COLOR_PANEL_BG
        ));

        // ── 标题：区域名 + 大能状态 ───────────────────────────────────────────
        String title = (zoneDisplayName == null || zoneDisplayName.isEmpty())
            ? "垂死大能"
            : zoneDisplayName + " · 垂死大能";
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.DYING_ELDER,
            title,
            panelX + 8,
            panelY + 5,
            COLOR_TITLE,
            0.75
        ));

        // ── 真元进度条 ────────────────────────────────────────────────────────
        int barX = panelX + QI_BAR_PADDING;
        int barY = panelY + 16;
        int barMaxWidth = PANEL_WIDTH - QI_BAR_PADDING * 2;
        int barFgWidth = Math.max(0, Math.round(barMaxWidth * Math.max(0f, Math.min(1f, qiFraction))));

        out.add(HudRenderCommand.rect(
            HudRenderLayer.DYING_ELDER,
            barX,
            barY,
            barMaxWidth,
            QI_BAR_HEIGHT,
            COLOR_QI_BAR_BG
        ));
        if (barFgWidth > 0) {
            out.add(HudRenderCommand.rect(
                HudRenderLayer.DYING_ELDER,
                barX,
                barY,
                barFgWidth,
                QI_BAR_HEIGHT,
                COLOR_QI_BAR_FG
            ));
        }

        // ── 背叛概率 / "气息有异" ─────────────────────────────────────────────
        String betrayText;
        int betrayColor;
        if (spiritEyeActive && betrayProbability != null) {
            betrayText = String.format(Locale.ROOT, "叛%.0f%%", betrayProbability * 100);
            betrayColor = betrayProbability >= 0.6 ? COLOR_BETRAY_HIGH : COLOR_BETRAY_LOW;
        } else {
            betrayText = "气息有异";
            betrayColor = COLOR_BETRAY_UNKNOWN;
        }
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.DYING_ELDER,
            betrayText,
            panelX + PANEL_WIDTH - 46,
            panelY + 5,
            betrayColor,
            0.75
        ));

        // ── 三个按钮：给丹 [G] / 拒绝 [H] / 拖延 [J] ────────────────────────
        int btnsTotalWidth = BTN_WIDTH * 3 + BTN_SPACING * 2;
        int btnsX = panelX + (PANEL_WIDTH - btnsTotalWidth) / 2;
        int btnsY = panelY + PANEL_HEIGHT - BTN_HEIGHT - 5;

        String[] labels = {"给丹 [G]", "拒绝 [H]", "拖延 [J]"};
        for (int i = 0; i < 3; i++) {
            int btnX = btnsX + i * (BTN_WIDTH + BTN_SPACING);
            out.add(HudRenderCommand.rect(
                HudRenderLayer.DYING_ELDER,
                btnX,
                btnsY,
                BTN_WIDTH,
                BTN_HEIGHT,
                COLOR_BTN_BG
            ));
            out.add(HudRenderCommand.scaledText(
                HudRenderLayer.DYING_ELDER,
                labels[i],
                btnX + 3,
                btnsY + 3,
                COLOR_BTN_TEXT,
                0.75
            ));
        }

        return out;
    }
}
