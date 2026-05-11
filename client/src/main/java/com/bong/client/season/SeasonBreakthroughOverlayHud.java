package com.bong.client.season;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.List;

public final class SeasonBreakthroughOverlayHud {
    private static final long PULSE_DURATION_MILLIS = 2_000L;

    private static ActivePulse activePulse = ActivePulse.empty();

    private SeasonBreakthroughOverlayHud() {
    }

    public static void trigger(SeasonBreakthroughOverlay.BreakthroughProfile profile, long nowMillis) {
        if (profile == null || (profile.screenPulseArgb() == 0 && profile.backlashIntensity() <= 0.0)) {
            return;
        }
        activePulse = new ActivePulse(
            profile.screenPulseArgb(),
            profile.backlashIntensity(),
            Math.max(0L, nowMillis),
            Math.max(0L, nowMillis) + PULSE_DURATION_MILLIS
        );
    }

    public static List<HudRenderCommand> buildCommands(long nowMillis) {
        ActivePulse pulse = activePulse;
        if (pulse.isExpired(nowMillis)) {
            activePulse = ActivePulse.empty();
            return List.of();
        }
        double progress = (double) (nowMillis - pulse.startedAtMillis()) / PULSE_DURATION_MILLIS;
        double envelope = Math.sin(Math.max(0.0, Math.min(1.0, progress)) * Math.PI);
        int tint = scaleAlpha(pulse.screenPulseArgb(), envelope);
        int vignette = scaleAlpha(0x30FF3355, envelope * pulse.backlashIntensity());
        if (tint == 0 && vignette == 0) {
            return List.of();
        }
        if (vignette == 0) {
            return List.of(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, tint));
        }
        if (tint == 0) {
            return List.of(HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, vignette));
        }
        return List.of(
            HudRenderCommand.screenTint(HudRenderLayer.VISUAL, tint),
            HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, vignette)
        );
    }

    static void resetForTests() {
        activePulse = ActivePulse.empty();
    }

    private static int scaleAlpha(int argb, double scale) {
        int alpha = (argb >>> 24) & 0xFF;
        int scaled = (int) Math.round(alpha * Math.max(0.0, Math.min(1.0, scale)));
        if (scaled <= 0) {
            return 0;
        }
        return (scaled << 24) | (argb & 0x00FFFFFF);
    }

    private record ActivePulse(
        int screenPulseArgb,
        double backlashIntensity,
        long startedAtMillis,
        long expiresAtMillis
    ) {
        static ActivePulse empty() {
            return new ActivePulse(0, 0.0, 0L, 0L);
        }

        ActivePulse {
            backlashIntensity = Math.max(0.0, Math.min(1.0, backlashIntensity));
        }

        boolean isExpired(long nowMillis) {
            return screenPulseArgb == 0 && backlashIntensity <= 0.0 || nowMillis >= expiresAtMillis;
        }
    }
}
