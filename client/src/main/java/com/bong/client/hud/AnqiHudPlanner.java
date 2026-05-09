package com.bong.client.hud;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class AnqiHudPlanner {
    private static final int AIM_COLOR = 0xFFE6D27A;
    private static final int CHARGE_COLOR = 0xFF8CE6FF;
    private static final int ECHO_COLOR = 0xFFB9A7FF;
    private static final int TEXT_COLOR = 0xFFEDE9D0;
    private static final int PANEL_BG = 0xB0121118;

    private AnqiHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        AnqiHudState state,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active(nowMillis) || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }
        float aimProgress = AnqiHudState.clamp01(state.aimProgress());
        float chargeProgress = AnqiHudState.clamp01(state.chargeProgress());
        if (aimProgress > 0f) appendAim(out, screenWidth, screenHeight, aimProgress);
        if (chargeProgress > 0f) appendCharge(out, screenWidth, screenHeight, chargeProgress);
        if (state.echoCount() > 0) appendEcho(out, screenWidth, screenHeight, state.echoCount());
        if (state.hasAbrasionContainer()) {
            appendAbrasion(out, screenWidth, screenHeight, state.abrasionContainer(), state.abrasionQiPayload());
        }
        return out;
    }

    private static void appendAim(List<HudRenderCommand> out, int w, int h, float progress) {
        int cx = w / 2;
        int cy = h / 2;
        int radius = Math.round(18 - 10 * progress);
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, cx - radius, cy, radius - 3, 1, AIM_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, cx + 3, cy, radius - 3, 1, AIM_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, cx, cy - radius, 1, radius - 3, AIM_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, cx, cy + 3, 1, radius - 3, AIM_COLOR));
    }

    private static void appendCharge(List<HudRenderCommand> out, int w, int h, float progress) {
        int barW = 96;
        int x = (w - barW) / 2;
        int y = h - 96;
        out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, barW, 5, 0xB0181820));
        out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, Math.round(barW * progress), 5, CHARGE_COLOR));
    }

    private static void appendEcho(List<HudRenderCommand> out, int w, int h, int count) {
        int x = w / 2 + 24;
        int y = h / 2 + 18;
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, x - 4, y - 3, 54, 14, PANEL_BG));
        out.add(HudRenderCommand.text(HudRenderLayer.CARRIER, "echo " + count, x, y, ECHO_COLOR));
    }

    private static void appendAbrasion(List<HudRenderCommand> out, int w, int h, String container, float qiPayload) {
        int x = w - 172;
        int y = h - 210;
        out.add(HudRenderCommand.rect(HudRenderLayer.CARRIER, x, y, 150, 26, PANEL_BG));
        String line = String.format(Locale.ROOT, "%s %.1f", container, qiPayload);
        out.add(HudRenderCommand.text(HudRenderLayer.CARRIER, line, x + 8, y + 9, TEXT_COLOR));
    }
}
