package com.bong.client.hud;

import com.bong.client.combat.CastState;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DefenseWindowState;

import java.util.ArrayList;
import java.util.List;

/**
 * Screen-edge feedback pulses (§5). Keeps the HUD otherwise quiet; emits
 * semi-transparent edge bands or full-screen tints for the handful of
 * hard-wired triggers listed in §5.2.
 *
 * <p>Because the existing render backend already has {@code EDGE_VIGNETTE} and
 * {@code SCREEN_TINT} commands, we reuse those instead of inventing a new
 * primitive. Rect commands are used for the 4-edge &ldquo;edge flash&rdquo;.
 */
public final class EdgeFeedbackHudPlanner {
    public static final float HP_LOW_THRESHOLD = 0.30f;
    public static final float HP_CRITICAL_THRESHOLD = 0.10f;
    public static final int FLASH_THICKNESS = 4;

    static final int HP_LOW_COLOR = 0x60FF4040;
    static final int HP_CRIT_COLOR = 0x90FF4040;
    static final int DEFENSE_WINDOW_COLOR = 0x70FF8080;
    static final int INTERRUPT_COLOR = 0x80FF4040;
    static final int PHASING_TINT = 0x26FF80FF; // ~15% alpha
    static final int TRIBULATION_VIGNETTE = 0xC0FF4040;

    private EdgeFeedbackHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        CombatHudState combat,
        DefenseWindowState defense,
        CastState cast,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) return out;
        if (combat == null) return out;

        // Persistent full-screen tints (bottom layer per §5.3)
        if (combat.active() && combat.derived().phasing()) {
            out.add(HudRenderCommand.screenTint(HudRenderLayer.EDGE_FEEDBACK, PHASING_TINT));
        }

        // Tribulation-locked vignette loop (§5.2 last row)
        if (combat.active() && combat.derived().tribulationLocked()) {
            out.add(HudRenderCommand.edgeVignette(HudRenderLayer.EDGE_FEEDBACK, TRIBULATION_VIGNETTE));
        }

        // Low-HP pulses (top layer above tint)
        if (combat.active()) {
            if (combat.hpPercent() <= HP_CRITICAL_THRESHOLD) {
                int alpha = pulseAlpha(nowMillis, 400L, 0.6f, 1.0f);
                out.add(HudRenderCommand.edgeVignette(HudRenderLayer.EDGE_FEEDBACK, blendAlpha(HP_CRIT_COLOR, alpha)));
            } else if (combat.hpPercent() <= HP_LOW_THRESHOLD) {
                int alpha = pulseAlpha(nowMillis, 800L, 0.3f, 0.7f);
                out.add(HudRenderCommand.edgeVignette(HudRenderLayer.EDGE_FEEDBACK, blendAlpha(HP_LOW_COLOR, alpha)));
            }
        }

        // DefenseWindow edge flash
        if (defense != null && defense.active() && !defense.isExpired(nowMillis)) {
            appendEdgeFlash(out, screenWidth, screenHeight, DEFENSE_WINDOW_COLOR);
        }

        // Cast interrupt flash (0.3s after ending)
        if (cast != null && cast.phase() == CastState.Phase.INTERRUPT) {
            long since = nowMillis - cast.endedAtMs();
            if (since >= 0 && since < 300L) {
                appendEdgeFlash(out, screenWidth, screenHeight, INTERRUPT_COLOR);
            }
        }

        return out;
    }

    private static void appendEdgeFlash(List<HudRenderCommand> out, int w, int h, int color) {
        out.add(HudRenderCommand.rect(HudRenderLayer.EDGE_FEEDBACK, 0, 0, w, FLASH_THICKNESS, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.EDGE_FEEDBACK, 0, h - FLASH_THICKNESS, w, FLASH_THICKNESS, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.EDGE_FEEDBACK, 0, 0, FLASH_THICKNESS, h, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.EDGE_FEEDBACK, w - FLASH_THICKNESS, 0, FLASH_THICKNESS, h, color));
    }

    private static int pulseAlpha(long nowMillis, long periodMs, float minFrac, float maxFrac) {
        double phase = (nowMillis % periodMs) / (double) periodMs;
        double sine = 0.5 * (1.0 - Math.cos(2.0 * Math.PI * phase));
        double frac = minFrac + (maxFrac - minFrac) * sine;
        return (int) Math.round(frac * 255.0);
    }

    private static int blendAlpha(int argb, int alpha) {
        int a = Math.max(0, Math.min(255, alpha));
        return (a << 24) | (argb & 0x00FFFFFF);
    }
}
