package com.bong.client.combat.juice;

public final class ParryDodgeJuicePlanner {
    private ParryDodgeJuicePlanner() {
    }

    public static ParryPlan parry(CombatJuiceEvent event, boolean perfect) {
        double[] back = normalizedBack(event.directionX(), event.directionZ());
        double scale = perfect ? 0.8 : 0.3;
        int ticks = perfect ? 2 : 1;
        int flashColor = perfect ? 0x80FFFFFF : 0x3350A0FF;
        String recipe = perfect ? "parry_perfect" : "parry_success";
        return new ParryPlan(
            new Pushback(event.attackerUuid(), back[0] * scale, back[1] * scale, ticks),
            new Pushback(event.targetUuid(), -back[0] * 0.3, -back[1] * 0.3, 1),
            perfect ? 6 : 3,
            flashColor,
            recipe,
            perfect
        );
    }

    public static DodgeGhost dodge(String entityUuid, int skinTint, long nowMs) {
        int rgb = skinTint == 0 ? 0x00C8C8C8 : skinTint & 0x00FFFFFF;
        return new DodgeGhost(entityUuid == null ? "" : entityUuid, 0x66_000000 | rgb, nowMs, 10);
    }

    private static double[] normalizedBack(double x, double z) {
        if (!Double.isFinite(x)) {
            x = 0.0;
        }
        if (!Double.isFinite(z)) {
            z = 0.0;
        }
        double len = Math.sqrt(x * x + z * z);
        if (len <= 1e-6) {
            return new double[] { 0.0, -1.0 };
        }
        return new double[] { -x / len, -z / len };
    }

    public record ParryPlan(
        Pushback attackerPushback,
        Pushback defenderPushback,
        int hitStopTicks,
        int screenFlashArgb,
        String audioRecipeId,
        boolean perfect
    ) {
    }

    public record Pushback(String entityUuid, double velocityX, double velocityZ, int ticks) {
    }

    public record DodgeGhost(String entityUuid, int argb, long startedAtMs, int durationTicks) {
        public float alphaAt(long nowMs) {
            long duration = Math.max(0, durationTicks) * 50L;
            if (duration == 0L) {
                return 0f;
            }
            long elapsed = Math.max(0L, nowMs - startedAtMs);
            if (elapsed >= duration) {
                return 0f;
            }
            return 0.4f * (1.0f - elapsed / (float) duration);
        }
    }
}
