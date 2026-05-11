package com.bong.client.combat.juice;

import java.util.ArrayList;
import java.util.List;

public record CombatJuiceProfile(
    CombatSchool school,
    CombatJuiceTier tier,
    int hitStopTicks,
    float shakeIntensity,
    int shakeDurationTicks,
    int qiColorArgb,
    int tintDurationTicks,
    float killSlowmoFactor,
    int killSlowmoTicks,
    boolean reverseShake,
    String audioRecipeId
) {
    public CombatJuiceProfile {
        school = school == null ? CombatSchool.GENERIC : school;
        tier = tier == null ? CombatJuiceTier.LIGHT : tier;
        hitStopTicks = Math.max(0, hitStopTicks);
        shakeIntensity = clamp01(shakeIntensity);
        shakeDurationTicks = Math.max(0, shakeDurationTicks);
        tintDurationTicks = Math.max(0, tintDurationTicks);
        killSlowmoFactor = clamp(killSlowmoFactor, 0.3f, 1.0f);
        killSlowmoTicks = Math.max(0, killSlowmoTicks);
        audioRecipeId = audioRecipeId == null ? "" : audioRecipeId;
    }

    public static CombatJuiceProfile select(CombatSchool school, CombatJuiceTier tier) {
        CombatSchool safeSchool = school == null ? CombatSchool.GENERIC : school;
        CombatJuiceTier safeTier = tier == null ? CombatJuiceTier.LIGHT : tier;
        if (safeSchool == CombatSchool.GENERIC) {
            return generic(safeTier);
        }
        return schoolProfile(safeSchool, safeTier);
    }

    public static List<CombatJuiceProfile> profiles() {
        List<CombatJuiceProfile> out = new ArrayList<>();
        for (CombatSchool school : CombatSchool.playableSchools()) {
            for (CombatJuiceTier tier : CombatJuiceTier.values()) {
                out.add(select(school, tier));
            }
        }
        return List.copyOf(out);
    }

    public int qiRgb() {
        return qiColorArgb & 0x00FFFFFF;
    }

    public long hitStopMillis() {
        return hitStopTicks * 50L;
    }

    public long tintMillis() {
        return tintDurationTicks * 50L;
    }

    public long shakeMillis() {
        return shakeDurationTicks * 50L;
    }

    public long killSlowmoMillis() {
        return killSlowmoTicks * 50L;
    }

    private static CombatJuiceProfile generic(CombatJuiceTier tier) {
        return switch (tier) {
            case LIGHT -> make(CombatSchool.GENERIC, tier, 2, 0.15f, 3, 6, 1.0f, 0, "hit_light");
            case HEAVY -> make(CombatSchool.GENERIC, tier, 5, 0.40f, 6, 10, 0.6f, 8, "hit_heavy");
            case CRITICAL -> make(CombatSchool.GENERIC, tier, 8, 0.70f, 10, 15, 0.3f, 16, "hit_critical");
        };
    }

    private static CombatJuiceProfile schoolProfile(CombatSchool school, CombatJuiceTier tier) {
        return switch (school) {
            case BAOMAI -> switch (tier) {
                case LIGHT -> make(school, tier, 3, 0.30f, 3, 6, 1.0f, 0, "baomai_hit_light");
                case HEAVY -> make(school, tier, 6, 0.60f, 8, 10, 0.6f, 8, "baomai_hit_heavy");
                case CRITICAL -> make(school, tier, 10, 0.90f, 12, 15, 0.3f, 16, "baomai_hit_critical");
            };
            case ANQI -> switch (tier) {
                case LIGHT -> make(school, tier, 1, 0.10f, 3, 6, 1.0f, 0, "dugu_hit_light");
                case HEAVY -> make(school, tier, 3, 0.25f, 5, 10, 0.7f, 6, "dugu_hit_heavy");
                case CRITICAL -> make(school, tier, 5, 0.50f, 8, 12, 0.4f, 12, "dugu_hit_critical");
            };
            case TUIKE -> switch (tier) {
                case LIGHT -> make(school, tier, 2, 0.15f, 3, 6, 1.0f, 0, "tuike_hit_light");
                case HEAVY -> make(school, tier, 4, 0.30f, 6, 10, 0.7f, 6, "tuike_hit_heavy");
                case CRITICAL -> make(school, tier, 7, 0.50f, 9, 12, 0.4f, 12, "tuike_hit_critical");
            };
            case WOLIU -> switch (tier) {
                case LIGHT -> make(school, tier, 2, 0.20f, 3, 6, 1.0f, 0, "woliu_hit_light");
                case HEAVY -> make(school, tier, 4, 0.40f, 7, 10, 0.6f, 8, "woliu_hit_heavy");
                case CRITICAL -> make(school, tier, 8, 0.70f, 10, 14, 0.3f, 16, "woliu_hit_critical");
            };
            case ZHENFA -> switch (tier) {
                case LIGHT -> make(school, tier, 2, 0.15f, 3, 6, 1.0f, 0, "zhenfa_hit_light");
                case HEAVY -> make(school, tier, 5, 0.40f, 6, 10, 0.6f, 8, "zhenfa_hit_heavy");
                case CRITICAL -> make(school, tier, 8, 0.60f, 10, 13, 0.35f, 14, "zhenfa_hit_critical");
            };
            case ZHENMAI -> switch (tier) {
                case LIGHT -> make(school, tier, 2, 0.20f, 3, 6, 1.0f, 0, "zhenmai_hit_light");
                case HEAVY -> make(school, tier, 4, 0.40f, 6, 10, 0.6f, 8, "zhenmai_hit_heavy");
                case CRITICAL -> make(school, tier, 6, 0.60f, 9, 12, 0.4f, 12, "zhenmai_hit_critical");
            };
            case DUGU -> switch (tier) {
                case LIGHT -> make(school, tier, 0, 0.00f, 0, 18, 1.0f, 0, "dugu_poison_hit_light");
                case HEAVY -> make(school, tier, 1, 0.05f, 4, 30, 0.8f, 4, "dugu_poison_hit_heavy");
                case CRITICAL -> make(school, tier, 2, 0.10f, 6, 45, 0.6f, 8, "dugu_poison_hit_critical");
            };
            case GENERIC -> generic(tier);
        };
    }

    private static CombatJuiceProfile make(
        CombatSchool school,
        CombatJuiceTier tier,
        int hitStopTicks,
        float shakeIntensity,
        int shakeDurationTicks,
        int tintDurationTicks,
        float slowmoFactor,
        int slowmoTicks,
        String audioRecipeId
    ) {
        return new CombatJuiceProfile(
            school,
            tier,
            hitStopTicks,
            shakeIntensity,
            shakeDurationTicks,
            school.qiColorArgb(),
            tintDurationTicks,
            slowmoFactor,
            slowmoTicks,
            school.reverseShake(),
            audioRecipeId
        );
    }

    private static float clamp01(float value) {
        return clamp(value, 0.0f, 1.0f);
    }

    private static float clamp(float value, float min, float max) {
        if (!Float.isFinite(value)) {
            return min;
        }
        return Math.max(min, Math.min(max, value));
    }
}
