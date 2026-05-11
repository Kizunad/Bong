package com.bong.client.combat.juice;

public final class CameraShakeController {
    public static final Offsets ZERO = new Offsets(0f, 0f);
    private static volatile Shake active = Shake.none();

    private CameraShakeController() {
    }

    public static Shake trigger(CombatJuiceProfile profile, double directionX, double directionZ, long nowMs) {
        if (profile == null || profile.shakeIntensity() <= 0f || profile.shakeDurationTicks() <= 0) {
            return Shake.none();
        }
        double[] perpendicular = perpendicular(directionX, directionZ, profile.reverseShake());
        Shake next = new Shake(
            profile.shakeIntensity(),
            profile.shakeDurationTicks(),
            perpendicular[0],
            perpendicular[1],
            profile.reverseShake(),
            Math.max(0L, nowMs)
        );
        active = next;
        return next;
    }

    public static Offsets activeOffsets(long nowMs) {
        Shake shake = active;
        if (shake == null || !shake.activeAt(nowMs)) {
            active = Shake.none();
            return ZERO;
        }
        return offsets(shake, nowMs);
    }

    public static Offsets offsets(Shake shake, long nowMs) {
        if (shake == null || !shake.activeAt(nowMs)) {
            return ZERO;
        }
        double ratio = shake.remainingRatioAt(nowMs);
        long elapsedTick = Math.max(0L, nowMs - shake.startedAtMs()) / 50L;
        double phase = switch ((int) (elapsedTick % 4L)) {
            case 0 -> 1.0;
            case 1 -> -0.85;
            case 2 -> 0.55;
            default -> -0.35;
        };
        double degrees = 2.0 * shake.intensity() * ratio * phase;
        return new Offsets((float) (shake.perpX() * degrees), (float) (Math.abs(shake.perpZ()) * degrees * 0.65));
    }

    public static double[] perpendicular(double directionX, double directionZ, boolean reverse) {
        if (!Double.isFinite(directionX)) {
            directionX = 0.0;
        }
        if (!Double.isFinite(directionZ)) {
            directionZ = 0.0;
        }
        double len = Math.sqrt(directionX * directionX + directionZ * directionZ);
        if (len <= 1e-6) {
            directionX = 0.0;
            directionZ = 1.0;
            len = 1.0;
        }
        double px = -directionZ / len;
        double pz = directionX / len;
        if (reverse) {
            px = -px;
            pz = -pz;
        }
        return new double[] { px, pz };
    }

    public static void resetForTests() {
        active = Shake.none();
    }

    public record Shake(
        float intensity,
        int durationTicks,
        double perpX,
        double perpZ,
        boolean reverse,
        long startedAtMs
    ) {
        public static Shake none() {
            return new Shake(0f, 0, 0.0, 0.0, false, 0L);
        }

        public long durationMillis() {
            return Math.max(0, durationTicks) * 50L;
        }

        public boolean activeAt(long nowMs) {
            return intensity > 0f && durationMillis() > 0L && nowMs - startedAtMs < durationMillis();
        }

        public double remainingRatioAt(long nowMs) {
            long duration = durationMillis();
            if (duration <= 0L) {
                return 0.0;
            }
            long elapsed = Math.max(0L, nowMs - startedAtMs);
            if (elapsed >= duration) {
                return 0.0;
            }
            return 1.0 - elapsed / (double) duration;
        }
    }

    public record Offsets(float yawDegrees, float pitchDegrees) {
        public boolean isZero() {
            return yawDegrees == 0f && pitchDegrees == 0f;
        }
    }
}
