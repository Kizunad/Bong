package com.bong.client.tiandao;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import java.util.ArrayList;
import java.util.List;

public final class TiandaoPresenceHudPlanner {
    private static final int SKY_CRACK_COLOR = 0x1F800000;
    private static final int SKY_CRACK_BRANCH_COLOR = 0x14601000;
    private static final int SKY_CRACK_DEEP_COLOR = 0x40601000;

    private TiandaoPresenceHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        TiandaoPresenceState state,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        if (state == null || !state.active()) {
            return List.of();
        }
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, state.vignetteArgb(nowMillis)));
        int tint = state.tintArgb();
        if (tint != 0) {
            out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, tint));
        }
        appendSkyCracks(out, state, screenWidth, screenHeight);
        if (state.shakeIntensity() > 0.0) {
            int alpha = (int) Math.round(48.0 * state.shakeIntensity());
            out.add(HudRenderCommand.rect(HudRenderLayer.EDGE_FEEDBACK, 0, 0, screenWidth, 1, (alpha << 24) | 0x601000));
            out.add(HudRenderCommand.rect(HudRenderLayer.EDGE_FEEDBACK, 0, screenHeight - 1, screenWidth, 1, (alpha << 24) | 0x601000));
        }
        return out;
    }

    private static void appendSkyCracks(
        List<HudRenderCommand> out,
        TiandaoPresenceState state,
        int screenWidth,
        int screenHeight
    ) {
        int crackCount = skyCrackCount(state.response());
        if (crackCount == 0 || screenWidth <= 0 || screenHeight <= 0) {
            return;
        }
        int topBand = Math.max(18, screenHeight / 4);
        int segment = Math.max(24, screenWidth / (crackCount + 1));
        for (int i = 0; i < crackCount; i++) {
            int anchorX = segment * (i + 1) - Math.max(0, segment / 5);
            int anchorY = Math.max(6, topBand / 5 + (i % 3) * Math.max(4, topBand / 9));
            int length = Math.max(12, segment / 3 + i * 3);
            boolean deep = "annihilate".equals(state.response());
            appendJaggedCrack(out, anchorX, anchorY, length, deep ? 2 : 1, deep);
        }
    }

    private static int skyCrackCount(String response) {
        return switch (response) {
            case "tribulation" -> 3;
            case "annihilate" -> 5;
            default -> 0;
        };
    }

    private static void appendJaggedCrack(
        List<HudRenderCommand> out,
        int x,
        int y,
        int length,
        int thickness,
        boolean deep
    ) {
        int mainA = Math.max(4, length / 3);
        int mainB = Math.max(4, length / 4);
        int mainC = Math.max(4, length - mainA - mainB);
        int mainColor = deep ? SKY_CRACK_DEEP_COLOR : SKY_CRACK_COLOR;
        out.add(HudRenderCommand.rect(HudRenderLayer.VISUAL, x, y, mainA, thickness, mainColor));
        out.add(HudRenderCommand.rect(
            HudRenderLayer.VISUAL,
            x + mainA - 1,
            y + thickness,
            thickness,
            mainB,
            mainColor
        ));
        out.add(HudRenderCommand.rect(
            HudRenderLayer.VISUAL,
            x + mainA - 1,
            y + mainB,
            mainC,
            thickness,
            mainColor
        ));
        out.add(HudRenderCommand.rect(
            HudRenderLayer.VISUAL,
            x + Math.max(2, mainA / 2),
            y + thickness,
            Math.max(5, mainB),
            thickness,
            SKY_CRACK_BRANCH_COLOR
        ));
    }
}
