package com.bong.client.hud;

import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.botany.BotanySkillStore;
import com.bong.client.botany.BotanySkillViewModel;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;

import java.util.ArrayList;
import java.util.List;

public final class BotanyHudPlanner {
    static final int PANEL_WIDTH = 340;
    static final int PANEL_HEIGHT = 176;
    static final int PANEL_BG = 0xD014141F;
    static final int PANEL_BORDER = 0xFF80C060;
    static final int PANEL_BORDER_INTERRUPTED = 0xFFFF7070;
    static final int PANEL_BORDER_COMPLETED = 0xFF80FF80;
    static final int TEXT_PRIMARY = 0xFFE8E8E8;
    static final int TEXT_MUTED = 0xFFAAAAAA;
    static final int TEXT_WARNING = 0xFFFFCC40;
    static final int TEXT_DANGER = 0xFFFF9090;
    static final int TRACK_BG = 0xFF202834;
    static final int PROGRESS_FILL = 0xFF80FF80;
    static final int XP_FILL = 0xFFC8A060;
    static final int BUTTON_MANUAL_BG = 0xFF1A2A18;
    static final int BUTTON_MANUAL_ACTIVE = 0xFF2F5630;
    static final int BUTTON_AUTO_BG = 0xFF2A2518;
    static final int BUTTON_AUTO_ACTIVE = 0xFF5A4A1A;
    static final int BUTTON_LOCKED_BG = 0xFF202020;
    static final int BUTTON_LOCKED_BORDER = 0xFF666666;

    private BotanyHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        return buildCommands(HarvestSessionStore.snapshot(), BotanySkillStore.snapshot(), widthMeasurer, screenWidth, screenHeight);
    }

    static List<HudRenderCommand> buildCommands(
        HarvestSessionViewModel session,
        BotanySkillViewModel skill,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (session == null || session.isEmpty() || widthMeasurer == null || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        BotanySkillViewModel safeSkill = skill == null ? BotanySkillViewModel.defaultView() : skill;
        int x = Math.max(12, Math.min(screenWidth - PANEL_WIDTH - 12, screenWidth / 2 + 28));
        int y = Math.max(24, Math.min(screenHeight - PANEL_HEIGHT - 24, screenHeight / 2 - 96));
        int borderColor = session.interrupted()
            ? PANEL_BORDER_INTERRUPTED
            : (session.completed() ? PANEL_BORDER_COMPLETED : PANEL_BORDER);

        appendPanel(out, x, y, borderColor);

        String title = HudTextHelper.clipToWidth("采集 · " + session.displayTargetName(), PANEL_WIDTH - 24, widthMeasurer);
        out.add(HudRenderCommand.text(HudRenderLayer.BOTANY, title, x + 12, y + 16, TEXT_PRIMARY));

        String subtitle = session.plantKindId().isEmpty() ? session.targetId() : session.plantKindId();
        String clippedSubtitle = HudTextHelper.clipToWidth(subtitle, PANEL_WIDTH - 24, widthMeasurer);
        if (!clippedSubtitle.isEmpty()) {
            out.add(HudRenderCommand.text(HudRenderLayer.BOTANY, clippedSubtitle, x + 12, y + 30, TEXT_MUTED));
        }

        String detail = session.detail().isEmpty()
            ? "轻量采集会话：权威进度由 bong:server_data 回填"
            : session.detail();
        String clippedDetail = HudTextHelper.clipToWidth(detail, PANEL_WIDTH - 24, widthMeasurer);
        out.add(HudRenderCommand.text(HudRenderLayer.BOTANY, clippedDetail, x + 12, y + 46, TEXT_MUTED));

        boolean autoUnlocked = safeSkill.autoUnlocked();
        appendModeButton(
            out,
            x + 12,
            y + 62,
            150,
            42,
            BotanyHarvestMode.MANUAL,
            true,
            session.mode() == BotanyHarvestMode.MANUAL,
            session.requestPending() && session.mode() == BotanyHarvestMode.MANUAL,
            session.mode() == BotanyHarvestMode.MANUAL ? "移动 / 受击断" : "任意玩家可用",
            TEXT_PRIMARY
        );
        appendModeButton(
            out,
            x + 178,
            y + 62,
            150,
            42,
            BotanyHarvestMode.AUTO,
            session.autoSelectable() && autoUnlocked,
            session.mode() == BotanyHarvestMode.AUTO,
            session.requestPending() && session.mode() == BotanyHarvestMode.AUTO,
            autoUnlocked ? "仅受击断 · 熟练加成" : ("需采药 Lv." + safeSkill.autoUnlockLevel()),
            autoUnlocked ? TEXT_WARNING : TEXT_MUTED
        );

        String progressLabel = progressLabel(session);
        out.add(HudRenderCommand.text(HudRenderLayer.BOTANY, progressLabel, x + 12, y + 124, statusColor(session)));
        appendBar(out, x + 12, y + 132, PANEL_WIDTH - 24, 10, TRACK_BG, PROGRESS_FILL, session.progress());

        String xpLabel = "采药经验 · Lv." + safeSkill.level() + "  " + safeSkill.xp() + " / " + safeSkill.xpToNextLevel();
        out.add(HudRenderCommand.text(
            HudRenderLayer.BOTANY,
            HudTextHelper.clipToWidth(xpLabel, PANEL_WIDTH - 24, widthMeasurer),
            x + 12,
            y + 154,
            TEXT_PRIMARY
        ));
        appendBar(out, x + 12, y + 160, PANEL_WIDTH - 24, 8, TRACK_BG, XP_FILL, safeSkill.progressRatio());
        return List.copyOf(out);
    }

    private static void appendPanel(List<HudRenderCommand> out, int x, int y, int borderColor) {
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, PANEL_WIDTH, PANEL_HEIGHT, PANEL_BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, PANEL_WIDTH, 1, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y + PANEL_HEIGHT - 1, PANEL_WIDTH, 1, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, 1, PANEL_HEIGHT, borderColor));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + PANEL_WIDTH - 1, y, 1, PANEL_HEIGHT, borderColor));
    }

    private static void appendModeButton(
        List<HudRenderCommand> out,
        int x,
        int y,
        int width,
        int height,
        BotanyHarvestMode mode,
        boolean enabled,
        boolean active,
        boolean pending,
        String subtitle,
        int subtitleColor
    ) {
        int bg = !enabled ? BUTTON_LOCKED_BG : (active ? (mode == BotanyHarvestMode.MANUAL ? BUTTON_MANUAL_ACTIVE : BUTTON_AUTO_ACTIVE)
            : (mode == BotanyHarvestMode.MANUAL ? BUTTON_MANUAL_BG : BUTTON_AUTO_BG));
        int border = !enabled ? BUTTON_LOCKED_BORDER : (active ? 0xFFFFFFFF : 0xFF7A8A70);
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, width, height, bg));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, width, 1, border));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y + height - 1, width, 1, border));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, 1, height, border));
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x + width - 1, y, 1, height, border));
        out.add(HudRenderCommand.text(HudRenderLayer.BOTANY, mode.keyLabel(), x + 8, y + 12, enabled ? TEXT_PRIMARY : TEXT_MUTED));
        out.add(HudRenderCommand.text(HudRenderLayer.BOTANY, mode.displayName(), x + 34, y + 12, enabled ? TEXT_PRIMARY : TEXT_MUTED));
        out.add(HudRenderCommand.text(HudRenderLayer.BOTANY, pending ? "请求中…" : subtitle, x + 8, y + 28, subtitleColor));
    }

    private static void appendBar(
        List<HudRenderCommand> out,
        int x,
        int y,
        int width,
        int height,
        int trackColor,
        int fillColor,
        double ratio
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, width, height, trackColor));
        int fillWidth = Math.max(0, Math.min(width, (int) Math.round(width * Math.max(0.0, Math.min(1.0, ratio)))));
        if (fillWidth > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.BOTANY, x, y, fillWidth, height, fillColor));
        }
    }

    private static String progressLabel(HarvestSessionViewModel session) {
        if (session.completed()) {
            return "采集完成 · 等待权威背包快照";
        }
        if (session.interrupted()) {
            return session.detail().isEmpty() ? "采集已打断" : ("采集已打断 · " + session.detail());
        }
        if (session.requestPending()) {
            return session.mode() == null ? "等待模式选择" : (session.mode().displayName() + "请求已发送");
        }
        if (session.mode() == null) {
            return "选择模式：E 手动 / R 自动";
        }
        return session.mode().displayName() + "进行中 · " + Math.round(session.progress() * 100.0) + "%";
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
