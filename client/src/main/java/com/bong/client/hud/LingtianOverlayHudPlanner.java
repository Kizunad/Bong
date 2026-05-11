package com.bong.client.hud;

import com.bong.client.lingtian.state.LingtianSessionStore;
import com.bong.client.state.SeasonState;

import java.util.ArrayList;
import java.util.List;

public final class LingtianOverlayHudPlanner {
    static final int PANEL_WIDTH = 116;
    static final int PANEL_HEIGHT = 34;
    static final int BG = 0xB0101810;
    static final int BORDER = 0xAA80D080;
    static final int TEXT = 0xFFE0F0D0;
    static final int WARNING = 0xFFFFB870;
    static final int TRACK = 0xFF203020;
    static final int FILL = 0xFF78D878;

    private LingtianOverlayHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(LingtianSessionStore.Snapshot snapshot, int screenWidth, int screenHeight) {
        return buildCommands(snapshot, screenWidth, screenHeight, null);
    }

    public static List<HudRenderCommand> buildCommands(
        LingtianSessionStore.Snapshot snapshot,
        int screenWidth,
        int screenHeight,
        SeasonState seasonState
    ) {
        LingtianSessionStore.Snapshot safeSnapshot = snapshot == null ? LingtianSessionStore.Snapshot.empty() : snapshot;
        if (!safeSnapshot.active() || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        float clampedProgress = clamp01(safeSnapshot.progress());
        int x = clampPosition(screenWidth / 2 + 18, screenWidth, PANEL_WIDTH);
        int y = clampPosition(screenHeight / 2 + 18, screenHeight, PANEL_HEIGHT);
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x, y, PANEL_WIDTH, PANEL_HEIGHT, BG));
        appendBorder(out, x, y, PANEL_WIDTH, PANEL_HEIGHT, BORDER);
        String plant = safeSnapshot.plantId() == null || safeSnapshot.plantId().isBlank() ? "地块" : safeSnapshot.plantId();
        out.add(HudRenderCommand.text(
            HudRenderLayer.LINGTIAN_OVERLAY,
            icon(safeSnapshot) + " " + plant + " " + Math.round(clampedProgress * 100.0f) + "%",
            x + 6,
            y + 5,
            safeSnapshot.dyeContaminationWarning() ? WARNING : TEXT
        ));
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x + 6, y + 20, PANEL_WIDTH - 12, 4, TRACK));
        appendSeasonIcon(out, seasonState, x + PANEL_WIDTH - 14, y + 19);
        int trackWidth = PANEL_WIDTH - 12;
        int progressWidth = Math.max(0, Math.min(trackWidth, Math.round(trackWidth * clampedProgress)));
        out.add(HudRenderCommand.rect(
            HudRenderLayer.LINGTIAN_OVERLAY,
            x + 6,
            y + 20,
            progressWidth,
            4,
            FILL
        ));
        int contamination = Math.round(Math.max(0.0f, Math.min(1.0f, safeSnapshot.dyeContamination())) * 100.0f);
        out.add(HudRenderCommand.text(
            HudRenderLayer.LINGTIAN_OVERLAY,
            "染 " + contamination + "%",
            x + 74,
            y + 24,
            safeSnapshot.dyeContaminationWarning() ? WARNING : 0xFFB8C8B0
        ));
        return List.copyOf(out);
    }

    static int seasonIconColor(SeasonState seasonState) {
        if (seasonState == null) {
            return 0;
        }
        return switch (seasonState.phase()) {
            case SUMMER -> 0x99FFB040;
            case WINTER -> 0x99E8F4FF;
            case SUMMER_TO_WINTER, WINTER_TO_SUMMER -> 0x999966CC;
        };
    }

    private static void appendSeasonIcon(List<HudRenderCommand> out, SeasonState seasonState, int x, int y) {
        int color = seasonIconColor(seasonState);
        if (color != 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x, y, 5, 5, color));
        }
    }

    private static String icon(LingtianSessionStore.Snapshot snapshot) {
        return switch (snapshot.kind()) {
            case TILL, RENEW -> "□";
            case PLANTING -> "+";
            case HARVEST -> "◆";
            case REPLENISH -> "◇";
            case DRAIN_QI -> "!";
        };
    }

    private static int clampPosition(int preferred, int screenSize, int panelSize) {
        int max = Math.max(0, screenSize - panelSize - 8);
        return Math.max(0, Math.min(preferred, max));
    }

    private static float clamp01(float value) {
        if (!Float.isFinite(value)) {
            return 0.0f;
        }
        return Math.max(0.0f, Math.min(1.0f, value));
    }

    private static void appendBorder(List<HudRenderCommand> out, int x, int y, int width, int height, int color) {
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x, y, width, 1, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x, y + height - 1, width, 1, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x, y, 1, height, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x + width - 1, y, 1, height, color));
    }
}
