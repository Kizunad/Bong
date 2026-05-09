package com.bong.client.hud;

import com.bong.client.yidao.YidaoHudStateStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** 医道支援流派 HUD：只展示医者档案、患者状态、业力和群体接经预览。 */
public final class YidaoHudPlanner {
    static final int PANEL_WIDTH = 176;
    static final int PANEL_HEIGHT = 64;
    static final int PANEL_BG = 0xB00B1712;
    static final int BORDER = 0xFF7ED7B1;
    static final int TEXT = 0xFFE6FFF4;
    static final int MUTED = 0xFFB6D7CA;
    static final int WARNING = 0xFFFFD37E;

    private YidaoHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        YidaoHudStateStore.Snapshot state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        YidaoHudStateStore.Snapshot safe = state == null ? YidaoHudStateStore.Snapshot.EMPTY : state;
        if (!safe.active() || widthMeasurer == null || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }

        int x = Math.max(8, screenWidth - PANEL_WIDTH - 10);
        int y = 34;
        int textRight = PANEL_WIDTH - 12;
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.YIDAO, x, y, PANEL_WIDTH, PANEL_HEIGHT, PANEL_BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.YIDAO, x, y, PANEL_WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.YIDAO, x, y + PANEL_HEIGHT - 1, PANEL_WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.YIDAO, x, y, 1, PANEL_HEIGHT, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.YIDAO, x + PANEL_WIDTH - 1, y, 1, PANEL_HEIGHT, BORDER));

        out.add(text(widthMeasurer, titleLine(safe), x + 6, y + 6, textRight, TEXT));
        out.add(text(widthMeasurer, identityLine(safe), x + 6, y + 18, textRight, MUTED));
        out.add(text(widthMeasurer, patientLine(safe), x + 6, y + 30, textRight, MUTED));
        out.add(text(widthMeasurer, karmaLine(safe), x + 6, y + 42, textRight, safe.karma() >= 10.0 ? WARNING : MUTED));
        return List.copyOf(out);
    }

    static String titleLine(YidaoHudStateStore.Snapshot state) {
        return "医道 " + skillLabel(state.activeSkill());
    }

    static String identityLine(YidaoHudStateStore.Snapshot state) {
        return String.format(Locale.ROOT, "信誉 %d  平和 %.0f", state.reputation(), state.peaceMastery());
    }

    static String patientLine(YidaoHudStateStore.Snapshot state) {
        String hp = state.patientHpPercent() == null
            ? "--"
            : String.format(Locale.ROOT, "%.0f%%", state.patientHpPercent() * 100f);
        String contam = state.patientContamTotal() == null
            ? "--"
            : String.format(Locale.ROOT, "%.1f", state.patientContamTotal());
        return "患者 " + state.patientIds().size()
            + "  HP " + hp
            + "  污染 " + contam
            + "  断脉 " + state.severedMeridianCount();
    }

    static String karmaLine(YidaoHudStateStore.Snapshot state) {
        return String.format(
            Locale.ROOT,
            "业力 %.1f  结契 %d  群体 %d",
            state.karma(),
            state.contractCount(),
            state.massPreviewCount()
        );
    }

    static String skillLabel(String skill) {
        return switch (skill == null ? "" : skill) {
            case "meridian_repair" -> "接经术";
            case "contam_purge" -> "排异";
            case "emergency_resuscitate" -> "急救";
            case "life_extension" -> "续命";
            case "mass_meridian_repair" -> "群体接经";
            default -> "待机";
        };
    }

    private static HudRenderCommand text(
        HudTextHelper.WidthMeasurer widthMeasurer,
        String text,
        int x,
        int y,
        int maxWidth,
        int color
    ) {
        return HudRenderCommand.text(
            HudRenderLayer.YIDAO,
            HudTextHelper.clipToWidth(text, maxWidth, widthMeasurer),
            x,
            y,
            color
        );
    }
}
