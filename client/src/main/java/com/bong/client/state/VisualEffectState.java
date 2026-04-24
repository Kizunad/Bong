package com.bong.client.state;

import java.util.Objects;
import java.util.Locale;

public final class VisualEffectState {
    private final EffectType effectType;
    private final double intensity;
    private final long durationMillis;
    private final long startedAtMillis;

    private VisualEffectState(EffectType effectType, double intensity, long durationMillis, long startedAtMillis) {
        this.effectType = Objects.requireNonNull(effectType, "effectType");
        this.intensity = intensity;
        this.durationMillis = durationMillis;
        this.startedAtMillis = startedAtMillis;
    }

    public static VisualEffectState none() {
        return new VisualEffectState(EffectType.NONE, 0.0, 0L, 0L);
    }

    public static VisualEffectState create(String effectType, double intensity, long durationMillis, long startedAtMillis) {
        EffectType normalizedType = EffectType.fromWireName(effectType);
        double normalizedIntensity = clamp(intensity, 0.0, 1.0);
        long normalizedDuration = Math.max(0L, durationMillis);
        long normalizedStart = Math.max(0L, startedAtMillis);

        if (normalizedType == EffectType.NONE || normalizedIntensity == 0.0 || normalizedDuration == 0L) {
            return none();
        }

        return new VisualEffectState(normalizedType, normalizedIntensity, normalizedDuration, normalizedStart);
    }

    private static double clamp(double value, double min, double max) {
        if (!Double.isFinite(value)) {
            return min;
        }
        return Math.max(min, Math.min(max, value));
    }

    public EffectType effectType() {
        return effectType;
    }

    public double intensity() {
        return intensity;
    }

    public long durationMillis() {
        return durationMillis;
    }

    public long startedAtMillis() {
        return startedAtMillis;
    }

    public boolean isEmpty() {
        return effectType == EffectType.NONE;
    }

    public boolean isActiveAt(long nowMillis) {
        return remainingRatioAt(nowMillis) > 0.0;
    }

    public double remainingRatioAt(long nowMillis) {
        if (isEmpty() || durationMillis == 0L) {
            return 0.0;
        }

        long safeNowMillis = Math.max(0L, nowMillis);
        long elapsedMillis = Math.max(0L, safeNowMillis - startedAtMillis);
        if (elapsedMillis >= durationMillis) {
            return 0.0;
        }
        return 1.0 - (elapsedMillis / (double) durationMillis);
    }

    public double scaledIntensityAt(long nowMillis) {
        return intensity * remainingRatioAt(nowMillis);
    }

    public enum EffectType {
        NONE("none"),
        SCREEN_SHAKE("screen_shake"),
        FOG_TINT("fog_tint"),
        TITLE_FLASH("title_flash"),
        BLOOD_MOON("blood_moon"),
        DEMONIC_FOG("demonic_fog"),
        ENLIGHTENMENT_FLASH("enlightenment_flash"),
        TRIBULATION_PRESSURE("tribulation_pressure"),
        FOV_ZOOM_IN("fov_zoom_in"),
        FOV_STRETCH("fov_stretch"),
        TRIBULATION_LOOK_UP("tribulation_look_up"),
        MEDITATION_CALM("meditation_calm"),
        POISON_TINT("poison_tint"),
        FROSTBITE("frostbite"),
        NEAR_DEATH_VIGNETTE("near_death_vignette"),
        PRESSURE_JITTER("pressure_jitter"),
        HIT_PUSHBACK("hit_pushback"),
        WEAPON_BREAK_FLASH("weapon_break_flash"),
        MEDITATION_INK_WASH("meditation_ink_wash");

        private final String wireName;

        EffectType(String wireName) {
            this.wireName = wireName;
        }

        public static EffectType fromWireName(String wireName) {
            String normalizedWireName = wireName == null ? "" : wireName.trim().toLowerCase(Locale.ROOT);
            return switch (normalizedWireName) {
                case "screen_shake", "camera_shake" -> SCREEN_SHAKE;
                case "fog_tint", "fog_pulse" -> FOG_TINT;
                case "title_flash", "title" -> TITLE_FLASH;
                case "blood_moon" -> BLOOD_MOON;
                case "demonic_fog", "demonic" -> DEMONIC_FOG;
                case "enlightenment_flash", "enlightenment" -> ENLIGHTENMENT_FLASH;
                case "tribulation_pressure", "tribulation" -> TRIBULATION_PRESSURE;
                case "fov_zoom_in", "fov_focus", "zoom_in" -> FOV_ZOOM_IN;
                case "fov_stretch", "fov_breakthrough", "stretch" -> FOV_STRETCH;
                case "tribulation_look_up", "look_up", "sky_gaze" -> TRIBULATION_LOOK_UP;
                case "meditation_calm", "meditation", "calm" -> MEDITATION_CALM;
                case "poison_tint", "poison" -> POISON_TINT;
                case "frostbite", "ice_poison", "freeze" -> FROSTBITE;
                case "near_death_vignette", "near_death", "low_hp" -> NEAR_DEATH_VIGNETTE;
                case "pressure_jitter", "pressure", "qi_pressure" -> PRESSURE_JITTER;
                case "hit_pushback", "pushback", "recoil", "knockback_cam" -> HIT_PUSHBACK;
                case "weapon_break_flash", "weapon_break", "break_flash" -> WEAPON_BREAK_FLASH;
                case "meditation_ink_wash", "ink_wash", "sumi_e", "flashback" -> MEDITATION_INK_WASH;
                default -> NONE;
            };
        }

        public String wireName() {
            return wireName;
        }
    }
}
