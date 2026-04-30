package com.bong.client.hud;

import com.bong.client.botany.BotanyDragState;
import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.botany.BotanySkillViewModel;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.bong.client.skill.SkillSetStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * plan-botany-v1 §1.3 采集浮窗。照 {@code docs/svg/harvest-popup.svg} 草图比例实现：
 * 460×340 草图 → 按 ~0.6 等比缩到 MC HUD 视口（280×204）。仅用 RECT + TEXT 原语，
 * 圆角/植物图标/品质分布暂用色块占位（见 §7 TODO）。
 *
 * <p>锚点：plan 要求"锚在植物方块屏幕投影附近，可拖拽"；此版最小化为"准星右下偏移"，
 * 让玩家视线中心保持可见，避免遮挡目标。真正的投影锚定 + 拖拽留待 target_entity 3D
 * 坐标上行后补。
 */
public final class BotanyHudPlanner {
    /**
     * canonical plant_kind → HUD thumbnail 资源。v1 22 种使用 botany 缩略图；
     * v2 绝地草木复用 inventory item icon，位于 {@code textures/gui/items/}。
     */
    static final Map<String, String> PLANT_ICON_PATHS = java.util.Map.ofEntries(
        java.util.Map.entry("ci_she_hao", "bong-client:textures/gui/botany/ci_she_hao.png"),
        java.util.Map.entry("ning_mai_cao", "bong-client:textures/gui/botany/ning_mai_cao.png"),
        java.util.Map.entry("hui_yuan_zhi", "bong-client:textures/gui/botany/hui_yuan_zhi.png"),
        java.util.Map.entry("chi_sui_cao", "bong-client:textures/gui/botany/chi_sui_cao.png"),
        java.util.Map.entry("gu_yuan_gen", "bong-client:textures/gui/botany/gu_yuan_gen.png"),
        java.util.Map.entry("kong_shou_hen", "bong-client:textures/gui/botany/kong_shou_hen.png"),
        java.util.Map.entry("jie_gu_rui", "bong-client:textures/gui/botany/jie_gu_rui.png"),
        java.util.Map.entry("yang_jing_tai", "bong-client:textures/gui/botany/yang_jing_tai.png"),
        java.util.Map.entry("qing_zhuo_cao", "bong-client:textures/gui/botany/qing_zhuo_cao.png"),
        java.util.Map.entry("an_shen_guo", "bong-client:textures/gui/botany/an_shen_guo.png"),
        java.util.Map.entry("shi_mai_gen", "bong-client:textures/gui/botany/shi_mai_gen.png"),
        java.util.Map.entry("ling_yan_shi_zhi", "bong-client:textures/gui/botany/ling_yan_shi_zhi.png"),
        java.util.Map.entry("ye_ku_teng", "bong-client:textures/gui/botany/ye_ku_teng.png"),
        java.util.Map.entry("hui_jin_tai", "bong-client:textures/gui/botany/hui_jin_tai.png"),
        java.util.Map.entry("zhen_jie_zi", "bong-client:textures/gui/botany/zhen_jie_zi.png"),
        java.util.Map.entry("shao_hou_man", "bong-client:textures/gui/botany/shao_hou_man.png"),
        java.util.Map.entry("tian_nu_jiao", "bong-client:textures/gui/botany/tian_nu_jiao.png"),
        java.util.Map.entry("fu_you_hua", "bong-client:textures/gui/botany/fu_you_hua.png"),
        java.util.Map.entry("wu_yan_guo", "bong-client:textures/gui/botany/wu_yan_guo.png"),
        java.util.Map.entry("hei_gu_jun", "bong-client:textures/gui/botany/hei_gu_jun.png"),
        java.util.Map.entry("fu_chen_cao", "bong-client:textures/gui/botany/fu_chen_cao.png"),
        java.util.Map.entry("zhong_yan_teng", "bong-client:textures/gui/botany/zhong_yan_teng.png"),
        java.util.Map.entry("fu_yuan_jue", "bong-client:textures/gui/items/fu_yuan_jue.png"),
        java.util.Map.entry("bai_yan_peng", "bong-client:textures/gui/items/bai_yan_peng.png"),
        java.util.Map.entry("duan_ji_ci", "bong-client:textures/gui/items/duan_ji_ci.png"),
        java.util.Map.entry("xue_se_mai_cao", "bong-client:textures/gui/items/xue_se_mai_cao.png"),
        java.util.Map.entry("yun_ding_lan", "bong-client:textures/gui/items/yun_ding_lan.png"),
        java.util.Map.entry("xuan_gen_wei", "bong-client:textures/gui/items/xuan_gen_wei.png"),
        java.util.Map.entry("ying_yuan_gu", "bong-client:textures/gui/items/ying_yuan_gu.png"),
        java.util.Map.entry("xuan_rong_tai", "bong-client:textures/gui/items/xuan_rong_tai.png"),
        java.util.Map.entry("yuan_ni_hong_yu", "bong-client:textures/gui/items/yuan_ni_hong_yu.png"),
        java.util.Map.entry("jing_xin_zao", "bong-client:textures/gui/items/jing_xin_zao.png"),
        java.util.Map.entry("xue_po_lian", "bong-client:textures/gui/items/xue_po_lian.png"),
        java.util.Map.entry("jiao_mai_teng", "bong-client:textures/gui/items/jiao_mai_teng.png"),
        java.util.Map.entry("lie_yuan_tai", "bong-client:textures/gui/items/lie_yuan_tai.png"),
        java.util.Map.entry("ming_gu_gu", "bong-client:textures/gui/items/ming_gu_gu.png"),
        java.util.Map.entry("bei_wen_zhi", "bong-client:textures/gui/items/bei_wen_zhi.png"),
        java.util.Map.entry("ling_jing_xu", "bong-client:textures/gui/items/ling_jing_xu.png"),
        java.util.Map.entry("mao_xin_wei", "bong-client:textures/gui/items/mao_xin_wei.png")
    );

    static final int PANEL_WIDTH = 280;
    static final int PANEL_HEIGHT = 204;
    static final int HEADER_HEIGHT = 24;
    static final int INFO_ROW_HEIGHT = 44;
    static final int BUTTON_WIDTH = 126;
    static final int BUTTON_HEIGHT = 48;
    static final int BUTTON_GAP = 8;
    static final int PANEL_PADDING = 10;

    static final int SHADOW_COLOR = 0x8C000000;
    static final int PANEL_BG = 0xE014141F;
    static final int HEADER_BG = 0xFF1A2A18;
    static final int PANEL_BORDER = 0xFF80C060;
    static final int PANEL_BORDER_INTERRUPTED = 0xFFFF7070;
    static final int PANEL_BORDER_COMPLETED = 0xFF80FF80;
    static final int TEXT_PRIMARY = 0xFFC0E0A0;
    static final int TEXT_BODY = 0xFFCCCCCC;
    static final int TEXT_MUTED = 0xFF888888;
    static final int TEXT_WARNING = 0xFFFFCC40;
    static final int TEXT_DANGER = 0xFFFF9090;
    static final int TEXT_HINT = 0xFFC8A878;
    static final int THUMB_BG = 0xFF0A0A14;
    static final int THUMB_BORDER = 0xFF3A4A30;
    static final int TRACK_BG = 0xFF001020;
    static final int TRACK_BORDER = 0xFF444444;
    static final int PROGRESS_FILL = 0xFF80FF80;
    static final int BUTTON_MANUAL_BG = 0xFF1A2A18;
    static final int BUTTON_MANUAL_BORDER = 0xFF80FF80;
    static final int BUTTON_AUTO_BG_LOCKED = 0xFF1A1A14;
    static final int BUTTON_AUTO_BORDER_LOCKED = 0xFF6A6050;
    static final int BUTTON_AUTO_BG_UNLOCKED = 0xFF2A2A18;
    static final int BUTTON_AUTO_BORDER_UNLOCKED = 0xFFFFCC40;

    private BotanyHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        return buildCommands(HarvestSessionStore.snapshot(), herbalismView(), widthMeasurer, screenWidth, screenHeight, null);
    }

    public static List<HudRenderCommand> buildCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        BotanyProjection.Anchor anchor
    ) {
        return buildCommands(HarvestSessionStore.snapshot(), herbalismView(), widthMeasurer, screenWidth, screenHeight, anchor);
    }

    public static BotanySkillViewModel herbalismView() {
        var entry = SkillSetStore.snapshot().get(com.bong.client.skill.SkillId.HERBALISM);
        return BotanySkillViewModel.create(entry.effectiveLv(), entry.xp(), entry.xpToNext(), 3);
    }

    static List<HudRenderCommand> buildCommands(
        HarvestSessionViewModel session,
        BotanySkillViewModel skill,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        return buildCommands(session, skill, widthMeasurer, screenWidth, screenHeight, null);
    }

    static List<HudRenderCommand> buildCommands(
        HarvestSessionViewModel session,
        BotanySkillViewModel skill,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        BotanyProjection.Anchor anchor
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (session == null || session.isEmpty() || widthMeasurer == null || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        BotanySkillViewModel safeSkill = skill == null ? BotanySkillViewModel.defaultView() : skill;

        // 锚点优先级：1) 目标植物屏幕投影（plan §1.3）；2) 失败时退回准星右侧。
        int anchorX;
        int anchorY;
        if (anchor != null && anchor.visible()) {
            // 植物屏幕位置右上方 24px 起 panel，让植物本体保留在 panel 左下视野里
            anchorX = anchor.x() + 24;
            anchorY = anchor.y() - PANEL_HEIGHT - 8;
        } else {
            anchorX = screenWidth / 2 + 36;
            anchorY = screenHeight / 2 - PANEL_HEIGHT / 2 - 16;
        }
        // 玩家拖拽偏移（plan §1.3 可拖拽）：叠加到 projection 基底上，session 切换归零。
        BotanyDragState.maybeResetForSession(session.sessionId());
        anchorX += BotanyDragState.deltaX();
        anchorY += BotanyDragState.deltaY();
        int x = Math.max(8, Math.min(screenWidth - PANEL_WIDTH - 8, anchorX));
        int y = Math.max(8, Math.min(screenHeight - PANEL_HEIGHT - 8, anchorY));
        BotanyDragState.recordRenderedBounds(x, y, PANEL_WIDTH, PANEL_HEIGHT);

        int borderColor = session.interrupted()
            ? PANEL_BORDER_INTERRUPTED
            : session.completed() ? PANEL_BORDER_COMPLETED : PANEL_BORDER;

        appendPanel(out, x, y, borderColor);
        appendHeader(out, session, x, y, widthMeasurer);

        int cursorY = y + HEADER_HEIGHT + PANEL_PADDING;
        appendInfoRow(out, session, x + PANEL_PADDING, cursorY, widthMeasurer);
        cursorY += INFO_ROW_HEIGHT + PANEL_PADDING;

        boolean autoUnlocked = safeSkill.autoUnlocked();
        appendManualButton(out, x + PANEL_PADDING, cursorY, session);
        appendAutoButton(out, x + PANEL_PADDING + BUTTON_WIDTH + BUTTON_GAP, cursorY, session, safeSkill, autoUnlocked);
        cursorY += BUTTON_HEIGHT + PANEL_PADDING;

        appendProgress(out, x + PANEL_PADDING, cursorY, session, widthMeasurer);
        cursorY += 22;

        appendFooter(out, x + PANEL_PADDING, cursorY, session, safeSkill, widthMeasurer);

        return List.copyOf(out);
    }

    private static void appendPanel(List<HudRenderCommand> out, int x, int y, int borderColor) {
        // 投影（1px 偏移暗色块）
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + 2, y + 2, PANEL_WIDTH, PANEL_HEIGHT, SHADOW_COLOR));
        // 主面板
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, PANEL_WIDTH, PANEL_HEIGHT, PANEL_BG));
        // 边框（四条 1px 实线）
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, PANEL_WIDTH, 1, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y + PANEL_HEIGHT - 1, PANEL_WIDTH, 1, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, 1, PANEL_HEIGHT, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + PANEL_WIDTH - 1, y, 1, PANEL_HEIGHT, borderColor));
    }

    private static void appendHeader(
        List<HudRenderCommand> out,
        HarvestSessionViewModel session,
        int x,
        int y,
        HudTextHelper.WidthMeasurer widthMeasurer
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + 1, y + 1, PANEL_WIDTH - 2, HEADER_HEIGHT - 1, HEADER_BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y + HEADER_HEIGHT, PANEL_WIDTH, 1, PANEL_BORDER));

        String title = HudTextHelper.clipToWidth("采集 · " + session.displayTargetName(),
            PANEL_WIDTH - PANEL_PADDING * 2 - 52, widthMeasurer);
        out.add(HudRenderCommand.text(HudRenderLayer.BOTANY, title, x + PANEL_PADDING, y + 8, TEXT_PRIMARY));

        String hint = session.interrupted() ? "已打断" : "ESC 取消";
        int hintWidth = widthMeasurer.measure(hint);
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            hint,
            x + PANEL_WIDTH - PANEL_PADDING - hintWidth,
            y + 8,
            session.interrupted() ? TEXT_DANGER : TEXT_MUTED
        ));
    }

    private static void appendInfoRow(
        List<HudRenderCommand> out,
        HarvestSessionViewModel session,
        int x,
        int y,
        HudTextHelper.WidthMeasurer widthMeasurer
    ) {
        // 缩略图：如有贴图 → 画纹理，边框保留；否则退回色块占位
        int thumbW = 40;
        int thumbH = 44;
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, thumbW, thumbH, THUMB_BG));
        String iconPath = PLANT_ICON_PATHS.get(session.plantKindId());
        if (iconPath != null) {
            out.add(HudRenderCommand.texture(HudRenderLayer.BOTANY, iconPath, x, y + 2, thumbW, thumbW, 0xFFFFFFFF));
        } else {
            out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + 16, y + 14, 8, 20, 0xFF4A6030));
            out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + 8, y + 18, 12, 4, 0xFF60A040));
            out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + 20, y + 22, 12, 4, 0xFF60A040));
            out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + 14, y + 10, 12, 4, 0xFF80C060));
        }
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, thumbW, 1, THUMB_BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y + thumbH - 1, thumbW, 1, THUMB_BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, 1, thumbH, THUMB_BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + thumbW - 1, y, 1, thumbH, THUMB_BORDER));

        int textX = x + thumbW + 10;
        int textRightLimit = PANEL_WIDTH - PANEL_PADDING * 2 - thumbW - 10;

        String kindLine = session.plantKindId().isEmpty() ? "—" : session.plantKindId();
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            HudTextHelper.clipToWidth(kindLine, textRightLimit, widthMeasurer),
            textX, y + 4, TEXT_BODY
        ));

        String detail = session.detail().isEmpty()
            ? "右键植物开始采集；选 E 或 R 启动模式"
            : session.detail();
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            HudTextHelper.clipToWidth(detail, textRightLimit, widthMeasurer),
            textX, y + 18, TEXT_MUTED
        ));

        String hazardLine = session.hazardHints().isEmpty()
            ? "采后灵气随你离 zone（plan §2 零和）"
            : "! " + session.hazardHints().get(0);
        int hazardColor = session.hazardHints().isEmpty() ? TEXT_HINT : TEXT_WARNING;
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            HudTextHelper.clipToWidth(hazardLine, textRightLimit, widthMeasurer),
            textX, y + 32, hazardColor
        ));
    }

    private static void appendManualButton(
        List<HudRenderCommand> out,
        int x,
        int y,
        HarvestSessionViewModel session
    ) {
        boolean active = session.mode() == BotanyHarvestMode.MANUAL;
        appendButtonBase(out, x, y, BUTTON_MANUAL_BG, BUTTON_MANUAL_BORDER, active);
        appendKeyChip(out, x + 4, y + 4, "E", BUTTON_MANUAL_BORDER);

        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            "手动采集",
            x + BUTTON_WIDTH / 2 - 18, y + 6, TEXT_PRIMARY
        ));

        String subtitle = session.requestPending() && active ? "请求中…" : "2.0s · 专注";
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            subtitle,
            x + BUTTON_WIDTH / 2 - 24, y + 20, TEXT_BODY
        ));

        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            "移动/受击即断",
            x + 6, y + 34, TEXT_MUTED
        ));
    }

    private static void appendAutoButton(
        List<HudRenderCommand> out,
        int x,
        int y,
        HarvestSessionViewModel session,
        BotanySkillViewModel skill,
        boolean unlocked
    ) {
        boolean enabled = session.autoSelectable() && unlocked;
        boolean active = session.mode() == BotanyHarvestMode.AUTO;
        int bg = enabled ? BUTTON_AUTO_BG_UNLOCKED : BUTTON_AUTO_BG_LOCKED;
        int border = enabled ? BUTTON_AUTO_BORDER_UNLOCKED : BUTTON_AUTO_BORDER_LOCKED;
        appendButtonBase(out, x, y, bg, border, active);
        appendKeyChip(out, x + 4, y + 4, "R", border);

        int titleColor = enabled ? TEXT_WARNING : TEXT_MUTED;
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            "自动采集",
            x + BUTTON_WIDTH / 2 - 18, y + 6, titleColor
        ));

        String subtitle;
        if (active && session.requestPending()) {
            subtitle = "请求中…";
        } else if (!enabled) {
            subtitle = "需采药 Lv." + skill.autoUnlockLevel();
        } else {
            subtitle = "5.0s · 仅受击断";
        }
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            subtitle,
            x + 6, y + 20, enabled ? TEXT_BODY : TEXT_WARNING
        ));

        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            enabled ? "XP + 熟练加成" : "磨练手动先攒经验",
            x + 6, y + 34, TEXT_MUTED
        ));
    }

    private static void appendButtonBase(
        List<HudRenderCommand> out,
        int x,
        int y,
        int bg,
        int border,
        boolean active
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, BUTTON_WIDTH, BUTTON_HEIGHT, bg));
        int borderColor = active ? 0xFFFFFFFF : border;
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, BUTTON_WIDTH, 1, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y + BUTTON_HEIGHT - 1, BUTTON_WIDTH, 1, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, 1, BUTTON_HEIGHT, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + BUTTON_WIDTH - 1, y, 1, BUTTON_HEIGHT, borderColor));
    }

    private static void appendKeyChip(List<HudRenderCommand> out, int x, int y, String label, int borderColor) {
        int w = 16;
        int h = 12;
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, w, h, 0xFF0A0A12));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, w, 1, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y + h - 1, w, 1, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, 1, h, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + w - 1, y, 1, h, borderColor));
        out.add(HudRenderCommand.text(HudRenderLayer.BOTANY, label, x + 5, y + 2, borderColor));
    }

    private static void appendProgress(
        List<HudRenderCommand> out,
        int x,
        int y,
        HarvestSessionViewModel session,
        HudTextHelper.WidthMeasurer widthMeasurer
    ) {
        int barWidth = PANEL_WIDTH - PANEL_PADDING * 2;
        int barHeight = 10;
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, barWidth, barHeight, TRACK_BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, barWidth, 1, TRACK_BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y + barHeight - 1, barWidth, 1, TRACK_BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, 1, barHeight, TRACK_BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + barWidth - 1, y, 1, barHeight, TRACK_BORDER));
        int fillWidth = Math.max(0, Math.min(barWidth - 2, (int) Math.round((barWidth - 2) * session.progress())));
        if (fillWidth > 0) {
            int fillColor = session.interrupted() ? TEXT_DANGER : PROGRESS_FILL;
            out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + 1, y + 1, fillWidth, barHeight - 2, fillColor));
        }

        String label = progressLabel(session);
        int labelWidth = widthMeasurer.measure(label);
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            label,
            x + barWidth - labelWidth, y - 10,
            statusColor(session)
        ));
    }

    private static void appendFooter(
        List<HudRenderCommand> out,
        int x,
        int y,
        HarvestSessionViewModel session,
        BotanySkillViewModel skill,
        HudTextHelper.WidthMeasurer widthMeasurer
    ) {
        int rightLimit = PANEL_WIDTH - PANEL_PADDING * 2;

        String breakLine = "打断 · WASD / 受击 / ESC";
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            HudTextHelper.clipToWidth(breakLine, rightLimit, widthMeasurer),
            x, y, TEXT_MUTED
        ));

        String trampleLine = session.interrupted()
            ? "本轮采集已中止"
            : "! 期间踩到植物 5% 会弄死它";
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            HudTextHelper.clipToWidth(trampleLine, rightLimit, widthMeasurer),
            x, y + 12, TEXT_DANGER
        ));

        String xpLine = "采药 Lv." + skill.level() + " · " + skill.xp() + "/" + skill.xpToNextLevel();
        int xpWidth = widthMeasurer.measure(xpLine);
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            xpLine,
            x + rightLimit - xpWidth, y + 24, TEXT_HINT
        ));
    }

    private static String progressLabel(HarvestSessionViewModel session) {
        if (session.completed()) {
            return "完成";
        }
        if (session.interrupted()) {
            return "中止";
        }
        if (session.requestPending()) {
            return session.mode() == null ? "待选模式" : "发送中…";
        }
        if (session.mode() == null) {
            return "选 E / R";
        }
        return Math.round(session.progress() * 100.0) + "%";
    }

    private static int statusColor(HarvestSessionViewModel session) {
        if (session.completed()) {
            return PANEL_BORDER_COMPLETED;
        }
        if (session.interrupted()) {
            return TEXT_DANGER;
        }
        if (session.requestPending()) {
            return TEXT_WARNING;
        }
        return TEXT_PRIMARY;
    }
}
