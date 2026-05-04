package com.bong.client.hud;

import com.bong.client.combat.store.CarrierStateStore;

import java.util.ArrayList;
import java.util.List;

/** Bottom-right anqi carrier charge / decay HUD. */
public final class CarrierHudPlanner {
    public static final int PANEL_WIDTH = 150;
    public static final int PANEL_HEIGHT = 42;
    private static final int RIGHT_MARGIN = 16;
    private static final int BOTTOM_MARGIN = 156;
    private static final int BG = 0xC0141816;
    private static final int BORDER = 0xFF7CB88A;
    private static final int TRACK_BG = 0xFF2A332D;
    private static final int CHARGE = 0xFF9EE6A8;
    private static final int DECAY = 0xFFE0C36A;
    private static final int TEXT = 0xFFE6F2E8;

    private CarrierHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        CarrierStateStore.State state,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active() || screenWidth <= 0 || screenHeight <= 0) return out;

        int x = screenWidth - PANEL_WIDTH - RIGHT_MARGIN;
        int y = screenHeight - PANEL_HEIGHT - BOTTOM_MARGIN;
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, x, y, PANEL_WIDTH, PANEL_HEIGHT, BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, x, y, PANEL_WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, x, y + PANEL_HEIGHT - 1, PANEL_WIDTH, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, x, y, 1, PANEL_HEIGHT, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, x + PANEL_WIDTH - 1, y, 1, PANEL_HEIGHT, BORDER));

        String label = state.phase() == CarrierStateStore.Phase.CHARGING ? "封骨" : "载体";
        out.add(HudRenderCommand.text(
            HudRenderLayer.CARRIER,
            label + " " + Math.round(state.sealedQi()) + "/" + Math.round(Math.max(state.sealedQiInitial(), state.sealedQi())),
            x + 8,
            y + 7,
            TEXT
        ));

        int trackX = x + 8;
        int trackY = y + 25;
        int trackW = PANEL_WIDTH - 16;
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, trackX, trackY, trackW, 5, TRACK_BG));
        int fill = Math.round(clamp01(state.progress()) * trackW);
        if (fill > 0) {
            out.add(HudRenderCommand.rect(
                HudRenderLayer.CARRIER,
                trackX,
                trackY,
                fill,
                5,
                state.phase() == CarrierStateStore.Phase.CHARGING ? CHARGE : DECAY
            ));
        }
        return out;
    }

    private static float clamp01(float v) {
        if (Float.isNaN(v)) return 0f;
        return Math.max(0f, Math.min(1f, v));
    }
}
