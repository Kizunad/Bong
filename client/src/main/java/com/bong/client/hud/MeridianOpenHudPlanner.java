package com.bong.client.hud;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.MeridianStateStore;

import java.util.ArrayList;
import java.util.List;

public final class MeridianOpenHudPlanner {
    private static final int PANEL_WIDTH = 200;
    private static final int PANEL_HEIGHT = 38;
    private static final int TRACK_HEIGHT = 4;
    private static final int BG = 0xD0111118;
    private static final int BORDER = 0xFF40C0E0;
    private static final int TEXT = 0xFFE6F3FF;
    private static final int MUTED = 0xFF8EA5B8;
    private static final int FILL = 0xFF40C0E0;

    private MeridianOpenHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0 || widthMeasurer == null) {
            return out;
        }
        MeridianBody body = MeridianStateStore.snapshot();
        if (body == null) return out;
        MeridianChannel target = body.targetMeridian();
        if (target == null) return out;
        ChannelState state = body.channel(target);
        if (state == null || !state.blocked()) return out;

        double progress = Math.max(0.0, Math.min(1.0, state.healProgress()));

        int x = Math.max(0, (screenWidth - PANEL_WIDTH) / 2);
        int y = Math.max(0, screenHeight - 82);

        out.add(HudRenderCommand.rect(HudRenderLayer.MERIDIAN_OPEN, x + 2, y + 2, PANEL_WIDTH, PANEL_HEIGHT, 0x88000000));
        out.add(HudRenderCommand.rect(HudRenderLayer.MERIDIAN_OPEN, x, y, PANEL_WIDTH, PANEL_HEIGHT, BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.MERIDIAN_OPEN, x, y, PANEL_WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.MERIDIAN_OPEN, x, y + PANEL_HEIGHT - 1, PANEL_WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.MERIDIAN_OPEN, x, y, 1, PANEL_HEIGHT, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.MERIDIAN_OPEN, x + PANEL_WIDTH - 1, y, 1, PANEL_HEIGHT, BORDER));

        String label = "冲脉 · " + target.displayName() + " " + Math.round(progress * 100) + "%";
        out.add(HudRenderCommand.text(
            HudRenderLayer.MERIDIAN_OPEN,
            HudTextHelper.clipToWidth(label, PANEL_WIDTH - 16, widthMeasurer),
            x + 8, y + 6, TEXT
        ));

        out.add(HudRenderCommand.text(
            HudRenderLayer.MERIDIAN_OPEN,
            "静坐吸灵中",
            x + 8, y + 18, MUTED
        ));

        int trackX = x + 8;
        int trackY = y + PANEL_HEIGHT - TRACK_HEIGHT - 5;
        int trackW = PANEL_WIDTH - 16;
        out.add(HudRenderCommand.rect(HudRenderLayer.MERIDIAN_OPEN, trackX, trackY, trackW, TRACK_HEIGHT, 0xFF101820));
        out.add(HudRenderCommand.rect(HudRenderLayer.MERIDIAN_OPEN, trackX, trackY, (int) Math.round(trackW * progress), TRACK_HEIGHT, FILL));

        return List.copyOf(out);
    }
}
