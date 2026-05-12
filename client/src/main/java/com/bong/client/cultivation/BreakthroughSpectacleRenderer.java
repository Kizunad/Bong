package com.bong.client.cultivation;

import com.bong.client.state.SeasonState;

import java.util.List;

public final class BreakthroughSpectacleRenderer {
    private BreakthroughSpectacleRenderer() {}

    public static SpectaclePlan plan(BreakthroughCinematicPayload payload, SeasonState season, long nowMillis) {
        if (payload == null) {
            return SpectaclePlan.empty();
        }
        SeasonState safeSeason = season == null ? SeasonState.summerAt(0L) : season;
        double seasonBoost = safeSeason.phase().tideTurn() ? 1.18 : safeSeason.phase() == SeasonState.Phase.WINTER ? 0.92 : 1.0;
        double intensity = clamp(payload.intensity() * seasonBoost, 0.0, 1.0);
        long durationMillis = Math.max(250L, payload.phaseDurationTicks() * 50L);

        return switch (payload.phase()) {
            case PRELUDE -> new SpectaclePlan(
                List.of("bong:cultivation_absorb"),
                "fov_zoom_in",
                intensity * 0.55,
                Math.min(durationMillis, 3_000L),
                "breakthrough_heartbeat_slow",
                "meditate_sit",
                "灵气开始聚拢",
                0xFF9FD3FF,
                payload.distantBillboard(),
                payload.style(),
                nowMillis
            );
            case CHARGE -> new SpectaclePlan(
                List.of("bong:cultivation_absorb", "bong:meridian_open"),
                safeSeason.phase().tideTurn() ? "pressure_jitter" : "fov_zoom_in",
                intensity * 0.72,
                Math.min(durationMillis, 5_000L),
                "breakthrough_heartbeat_fast",
                "meditate_sit_charge",
                "经脉光路循环",
                0xFF88CCDD,
                payload.distantBillboard(),
                payload.style(),
                nowMillis
            );
            case CATALYZE -> new SpectaclePlan(
                List.of("bong:breakthrough_pillar"),
                payload.global() ? "tribulation_look_up" : "fov_stretch",
                intensity * 0.86,
                Math.min(durationMillis, 5_000L),
                "breakthrough_resonance_hum",
                "breakthrough_" + payload.realmTo().toLowerCase(),
                "光柱升起",
                0xFFFFF3B0,
                payload.distantBillboard(),
                payload.style(),
                nowMillis
            );
            case APEX -> new SpectaclePlan(
                List.of("bong:breakthrough_pillar"),
                payload.global() || "void_tribulation".equals(payload.style()) ? "tribulation_pressure" : "enlightenment_flash",
                intensity,
                Math.min(durationMillis, 2_500L),
                "breakthrough_bell",
                "breakthrough_apex",
                payload.realmTo() + " 突破临界",
                0xFFFFD700,
                payload.distantBillboard(),
                payload.style(),
                nowMillis
            );
            case AFTERMATH -> aftermathPlan(payload, intensity, durationMillis, nowMillis);
        };
    }

    private static SpectaclePlan aftermathPlan(
        BreakthroughCinematicPayload payload,
        double intensity,
        long durationMillis,
        long nowMillis
    ) {
        if (payload.result().failed() || payload.interrupted()) {
            return new SpectaclePlan(
                List.of("bong:breakthrough_fail"),
                "screen_shake",
                Math.max(0.5, intensity),
                Math.min(durationMillis, 1_500L),
                payload.interrupted() ? "breakthrough_interrupted" : "breakthrough_failure",
                "hurt_stagger",
                payload.interrupted() ? "突破被打断" : "突破失败",
                0xFFFF5555,
                payload.distantBillboard(),
                payload.style(),
                nowMillis
            );
        }
        return new SpectaclePlan(
            List.of("bong:breakthrough_pillar"),
            "title_flash",
            Math.max(0.45, intensity * 0.7),
            Math.min(durationMillis, 4_000L),
            "breakthrough_afterglow",
            "breakthrough_success",
            payload.realmTo() + " 成就",
            0xFFFFD700,
            payload.distantBillboard(),
            payload.style(),
            nowMillis
        );
    }

    private static double clamp(double value, double min, double max) {
        if (!Double.isFinite(value)) return min;
        return Math.max(min, Math.min(max, value));
    }

    public record SpectaclePlan(
        List<String> vfxEventIds,
        String visualEffectType,
        double visualIntensity,
        long visualDurationMillis,
        String audioRecipeId,
        String animationId,
        String toastText,
        int toastColor,
        boolean distantBillboard,
        String style,
        long plannedAtMillis
    ) {
        public SpectaclePlan {
            vfxEventIds = vfxEventIds == null ? List.of() : List.copyOf(vfxEventIds);
            visualEffectType = visualEffectType == null ? "none" : visualEffectType;
            visualIntensity = clamp(visualIntensity, 0.0, 1.0);
            visualDurationMillis = Math.max(0L, visualDurationMillis);
            audioRecipeId = audioRecipeId == null ? "" : audioRecipeId;
            animationId = animationId == null ? "" : animationId;
            toastText = toastText == null ? "" : toastText;
            style = style == null ? "" : style;
            plannedAtMillis = Math.max(0L, plannedAtMillis);
        }

        public static SpectaclePlan empty() {
            return new SpectaclePlan(List.of(), "none", 0.0, 0L, "", "", "", 0, false, "", 0L);
        }
    }
}
