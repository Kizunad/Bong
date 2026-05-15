package com.bong.client.hud;

import com.bong.client.combat.store.VortexStateStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** Right-side woliu status panel. */
public final class WoliuV2StatusPanelHud {
    private static final int PANEL_MIN_WIDTH = 150;
    private static final int PANEL_MAX_WIDTH = 190;
    private static final int PANEL_HEIGHT = 86;
    private static final int TRACK_HEIGHT = 4;
    private static final int BG = 0xAA071018;
    private static final int BORDER = 0x7747D6E8;
    private static final int TEXT = 0xFFE7F7FF;
    private static final int MUTED = 0xFF9AB8C8;
    private static final int ACCENT = 0xFF62D6E8;
    private static final int WARNING = 0xFFFFB268;

    private WoliuV2StatusPanelHud() {
    }

    public static List<HudRenderCommand> buildCommands(
        VortexStateStore.State state,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        if (state == null || screenWidth <= 0 || screenHeight <= 0) return List.of();

        boolean skillActive = state.active() && !state.activeSkillId().isBlank();
        boolean turbulenceVisible = hasVisibleTurbulence(state, nowMillis);
        long cooldownMs = Math.max(0L, state.cooldownUntilMs() - nowMillis);
        boolean backfire = skillActive && !state.backfireLevel().isBlank();
        if (!skillActive && !turbulenceVisible && cooldownMs <= 0L && !backfire) {
            return List.of();
        }

        int panelW = clamp(Math.round(screenWidth * 0.22f), PANEL_MIN_WIDTH, PANEL_MAX_WIDTH);
        int margin = clamp(Math.round(screenWidth * 0.025f), 10, 24);
        int x = Math.max(8, screenWidth - panelW - margin);
        int y = clamp(Math.round(screenHeight * 0.28f), 38, Math.max(38, screenHeight - PANEL_HEIGHT - 24));
        int innerX = x + 8;
        int barW = panelW - 16;

        List<HudRenderCommand> out = new ArrayList<>();
        appendPanel(out, x, y, panelW, PANEL_HEIGHT);
        out.add(HudRenderCommand.text(HudRenderLayer.VORTEX_TURBULENCE, "涡流", innerX, y + 6, TEXT));
        out.add(HudRenderCommand.text(
            HudRenderLayer.VORTEX_TURBULENCE,
            statusLabel(skillActive, turbulenceVisible, cooldownMs),
            x + panelW - 48,
            y + 6,
            turbulenceVisible ? ACCENT : MUTED
        ));

        int lineY = y + 20;
        out.add(HudRenderCommand.text(
            HudRenderLayer.VORTEX_TURBULENCE,
            skillActive ? skillName(state.activeSkillId()) : "待机",
            innerX,
            lineY,
            skillActive ? TEXT : MUTED
        ));
        if (cooldownMs > 0L) {
            out.add(HudRenderCommand.text(
                HudRenderLayer.VORTEX_TURBULENCE,
                "冷却 " + secondsText(cooldownMs),
                x + panelW - 62,
                lineY,
                MUTED
            ));
        }

        int barY = y + 35;
        out.add(HudRenderCommand.rect(HudRenderLayer.VORTEX_TURBULENCE, innerX, barY, barW, TRACK_HEIGHT, 0xCC111722));
        if (skillActive) {
            float progress = Math.max(0f, Math.min(1f, state.chargeProgress()));
            int fill = Math.round(barW * progress);
            if (fill > 0) {
                out.add(HudRenderCommand.rect(HudRenderLayer.VORTEX_TURBULENCE, innerX, barY, fill, TRACK_HEIGHT, ACCENT));
            }
        }

        out.add(HudRenderCommand.text(
            HudRenderLayer.VORTEX_TURBULENCE,
            "半径 " + oneDecimal(Math.max(state.radius(), state.turbulenceRadius())) + "  强度 "
                + Math.round(state.turbulenceIntensity() * 100f) + "%",
            innerX,
            y + 45,
            turbulenceVisible ? TEXT : MUTED
        ));
        out.add(HudRenderCommand.text(
            HudRenderLayer.VORTEX_TURBULENCE,
            "拦截 " + state.interceptedCount() + backfireText(state),
            innerX,
            y + 58,
            backfire ? WARNING : MUTED
        ));
        if (turbulenceVisible) {
            out.add(HudRenderCommand.text(
                HudRenderLayer.VORTEX_TURBULENCE,
                "紊流 " + secondsText(state.turbulenceUntilMs() - nowMillis),
                innerX,
                y + 70,
                ACCENT
            ));
        }
        return List.copyOf(out);
    }

    static boolean hasVisibleTurbulence(VortexStateStore.State state, long nowMillis) {
        return state != null
            && state.active()
            && state.turbulenceRadius() > 0f
            && state.turbulenceIntensity() > 0f
            && state.turbulenceUntilMs() > nowMillis;
    }

    private static void appendPanel(List<HudRenderCommand> out, int x, int y, int w, int h) {
        out.add(HudRenderCommand.rect(HudRenderLayer.VORTEX_TURBULENCE, x, y, w, h, BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.VORTEX_TURBULENCE, x, y, w, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.VORTEX_TURBULENCE, x, y + h - 1, w, 1, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.VORTEX_TURBULENCE, x, y, 1, h, BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.VORTEX_TURBULENCE, x + w - 1, y, 1, h, BORDER));
    }

    private static String statusLabel(boolean skillActive, boolean turbulenceVisible, long cooldownMs) {
        if (skillActive) return "施放";
        if (turbulenceVisible) return "紊流";
        if (cooldownMs > 0L) return "冷却";
        return "待机";
    }

    private static String skillName(String id) {
        return switch (id) {
            case "woliu.hold" -> "持涡";
            case "woliu.burst" -> "涡爆";
            case "woliu.mouth" -> "涡口";
            case "woliu.pull" -> "牵涡";
            case "woliu.heart" -> "涡心";
            case "woliu.vacuum_palm" -> "真空掌";
            case "woliu.vortex_shield" -> "涡盾";
            case "woliu.vacuum_lock" -> "真空锁";
            case "woliu.vortex_resonance" -> "涡流共振";
            case "woliu.turbulence_burst" -> "紊流爆发";
            default -> id == null || id.isBlank() ? "待机" : id;
        };
    }

    private static String backfireText(VortexStateStore.State state) {
        if (state.backfireLevel().isBlank()) return "";
        return "  反噬 " + state.backfireLevel();
    }

    private static String secondsText(long millis) {
        long seconds = Math.max(0L, millis) / 1000L + (millis % 1000L == 0L ? 0L : 1L);
        return Math.min(Integer.MAX_VALUE, seconds) + "s";
    }

    private static String oneDecimal(float value) {
        return String.format(Locale.ROOT, "%.1f", Math.max(0f, value));
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
