package com.bong.client.npc;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudTextHelper;

import java.util.ArrayList;
import java.util.List;

public final class NpcInteractionLogHudPlanner {
    private static final int WIDTH = 190;
    private static final int ROW_HEIGHT = 12;
    private static final int BG = 0xCC101018;
    private static final int BORDER = 0xAAE8D090;
    private static final int TEXT = 0xFFECE8D8;
    private static final int MUTED = 0xFFAAA8A0;

    private NpcInteractionLogHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        List<NpcInteractionLogEntry> entries,
        boolean visible,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        if (!visible || entries == null || entries.isEmpty() || widthMeasurer == null || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        List<NpcInteractionLogEntry> capped = entries.stream().limit(NpcInteractionLogStore.MAX_ENTRIES).toList();
        int x = Math.max(8, screenWidth - WIDTH - 10);
        int y = Math.max(28, screenHeight / 2 - (capped.size() * ROW_HEIGHT) / 2);
        int height = 16 + capped.size() * ROW_HEIGHT;
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y, WIDTH, height, BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y, WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y + height - 1, WIDTH, 1, BORDER));
        out.add(HudRenderCommand.text(HudRenderLayer.TARGET_INFO, "交互记录", x + 6, y + 5, TEXT));
        int rowY = y + 18;
        for (NpcInteractionLogEntry entry : capped) {
            String name = HudTextHelper.clipToWidth(entry.displayName(), 92, widthMeasurer);
            String type = HudTextHelper.clipToWidth(labelFor(entry.interactionType()), 58, widthMeasurer);
            out.add(HudRenderCommand.text(HudRenderLayer.TARGET_INFO, name, x + 6, rowY, TEXT));
            out.add(HudRenderCommand.text(HudRenderLayer.TARGET_INFO, type, x + 124, rowY, MUTED));
            rowY += ROW_HEIGHT;
        }
        return List.copyOf(out);
    }

    static String labelFor(String interactionType) {
        return switch (interactionType == null ? "" : interactionType) {
            case "greeting" -> "招呼";
            case "reaction" -> "反应";
            case "warning" -> "预兆";
            case "memory" -> "记忆";
            case "trade" -> "交易";
            default -> "交互";
        };
    }
}
