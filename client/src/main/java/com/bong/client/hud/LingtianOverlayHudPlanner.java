package com.bong.client.hud;

import com.bong.client.lingtian.state.LingtianSessionStore;

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
        LingtianSessionStore.Snapshot safeSnapshot = snapshot == null ? LingtianSessionStore.Snapshot.empty() : snapshot;
        if (!safeSnapshot.active() || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        int x = Math.min(screenWidth - PANEL_WIDTH - 8, screenWidth / 2 + 18);
        int y = Math.min(screenHeight - PANEL_HEIGHT - 8, screenHeight / 2 + 18);
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x, y, PANEL_WIDTH, PANEL_HEIGHT, BG));
        appendBorder(out, x, y, PANEL_WIDTH, PANEL_HEIGHT, BORDER);
        String plant = safeSnapshot.plantId() == null || safeSnapshot.plantId().isBlank() ? "地块" : safeSnapshot.plantId();
        out.add(HudRenderCommand.text(
            HudRenderLayer.LINGTIAN_OVERLAY,
            icon(safeSnapshot) + " " + plant + " " + Math.round(safeSnapshot.progress() * 100.0f) + "%",
            x + 6,
            y + 5,
            safeSnapshot.dyeContaminationWarning() ? WARNING : TEXT
        ));
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x + 6, y + 20, PANEL_WIDTH - 12, 4, TRACK));
        out.add(HudRenderCommand.rect(
            HudRenderLayer.LINGTIAN_OVERLAY,
            x + 6,
            y + 20,
            Math.max(0, Math.round((PANEL_WIDTH - 12) * safeSnapshot.progress())),
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

    private static String icon(LingtianSessionStore.Snapshot snapshot) {
        return switch (snapshot.kind()) {
            case TILL, RENEW -> "□";
            case PLANTING -> "+";
            case HARVEST -> "◆";
            case REPLENISH -> "◇";
            case DRAIN_QI -> "!";
        };
    }

    private static void appendBorder(List<HudRenderCommand> out, int x, int y, int width, int height, int color) {
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x, y, width, 1, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x, y + height - 1, width, 1, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x, y, 1, height, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.LINGTIAN_OVERLAY, x + width - 1, y, 1, height, color));
    }
}
