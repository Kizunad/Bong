package com.bong.client.hud;

import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.npc.NpcMoodState;
import com.bong.client.npc.NpcMoodStore;
import com.bong.client.npc.ThreatAssessmentBar;

import java.util.ArrayList;
import java.util.List;

public final class TargetInfoHudPlanner {
    static final int PANEL_WIDTH = 220;
    static final int PANEL_HEIGHT = 34;
    static final int TRACK_HEIGHT = 4;
    static final int TEXT_COLOR = 0xFFECE8D8;
    static final int MUTED_COLOR = 0xFFAAA8A0;
    static final int HP_COLOR = 0xFFE06058;
    static final int QI_COLOR = 0xFF62B8FF;
    static final int BG_COLOR = 0xB0101018;
    static final int BORDER_COLOR = 0xAAE8D090;

    private TargetInfoHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        TargetInfoState state,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        TargetInfoState safeState = state == null ? TargetInfoState.empty() : state;
        if (!safeState.activeAt(nowMillis) || widthMeasurer == null || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }

        int alpha = safeState.alphaAt(nowMillis);
        if (alpha <= 0) {
            return List.of();
        }

        PlayerStateViewModel viewer = PlayerStateStore.snapshot();
        NpcMoodState npcMood = safeState.kind() == TargetInfoState.Kind.NPC
            ? NpcMoodStore.get(entityIdFromTargetId(safeState.targetId()))
            : null;
        boolean showThreatBar = ThreatAssessmentBar.visibleFor(viewer, npcMood);
        int panelHeight = showThreatBar ? PANEL_HEIGHT + 18 : PANEL_HEIGHT;
        if (showThreatBar && npcMood.innerMonologue() != null) {
            panelHeight += 12;
        }

        int x = Math.max(8, (screenWidth - PANEL_WIDTH) / 2);
        int y = 16;
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y, PANEL_WIDTH, panelHeight, withAlpha(BG_COLOR, alpha)));
        appendBorder(out, x, y, PANEL_WIDTH, panelHeight, withAlpha(BORDER_COLOR, alpha));

        String name = HudTextHelper.clipToWidth(safeState.displayName(), PANEL_WIDTH - 12, widthMeasurer);
        out.add(HudRenderCommand.text(HudRenderLayer.TARGET_INFO, name, x + 6, y + 5, withAlpha(TEXT_COLOR, alpha)));

        String realm = safeState.realmText(viewer);
        if (!realm.isEmpty()) {
            int realmWidth = widthMeasurer.measure(realm);
            out.add(HudRenderCommand.text(
                HudRenderLayer.TARGET_INFO,
                realm,
                x + PANEL_WIDTH - 6 - realmWidth,
                y + 5,
                withAlpha(MUTED_COLOR, alpha)
            ));
        }

        if (safeState.kind() != TargetInfoState.Kind.PLAYER) {
            appendBar(out, x + 6, y + 20, PANEL_WIDTH - 12, safeState.hpRatio(), HP_COLOR, alpha);
            if (safeState.kind() == TargetInfoState.Kind.NPC && safeState.revealRealm(viewer)) {
                appendBar(out, x + 6, y + 26, PANEL_WIDTH - 12, safeState.qiRatio(), QI_COLOR, alpha);
            }
            if (showThreatBar) {
                out.addAll(ThreatAssessmentBar.buildCommands(
                    npcMood,
                    viewer,
                    x + 6,
                    y + 33,
                    alpha,
                    widthMeasurer
                ));
            }
        }
        return List.copyOf(out);
    }

    private static int entityIdFromTargetId(String targetId) {
        if (targetId == null) {
            return -1;
        }
        int colon = targetId.lastIndexOf(':');
        String raw = colon >= 0 ? targetId.substring(colon + 1) : targetId;
        try {
            return Integer.parseInt(raw);
        } catch (NumberFormatException exception) {
            return -1;
        }
    }

    private static void appendBar(
        List<HudRenderCommand> out,
        int x,
        int y,
        int width,
        double ratio,
        int color,
        int alpha
    ) {
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y, width, TRACK_HEIGHT, withAlpha(0xD0000000, alpha)));
        int fill = Math.max(0, Math.min(width, (int) Math.round(width * ratio)));
        if (fill > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y, fill, TRACK_HEIGHT, withAlpha(color, alpha)));
        }
    }

    private static void appendBorder(List<HudRenderCommand> out, int x, int y, int width, int height, int color) {
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y, width, 1, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y + height - 1, width, 1, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y, 1, height, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x + width - 1, y, 1, height, color));
    }

    private static int withAlpha(int color, int alpha) {
        return HudTextHelper.withAlpha(color, alpha);
    }
}
