package com.bong.client.season;

public final class MigrationVisualPlanner {
    private MigrationVisualPlanner() {
    }

    public static MigrationVisualCommand plan(MigrationVisualEvent event, long nowTick) {
        if (event == null || nowTick < event.startedAtTick() || nowTick > event.startedAtTick() + event.durationTicks()) {
            return MigrationVisualCommand.none();
        }
        double progress = (double) (nowTick - event.startedAtTick()) / Math.max(1.0, event.durationTicks());
        int dustPerEntityPerFiveTicks = Math.max(1, Math.min(8, (int) Math.ceil(event.entityCount() / 12.0)));
        double shake = Math.min(0.05, 0.015 + event.entityCount() * 0.0015) * fade(progress);
        return new MigrationVisualCommand(dustPerEntityPerFiveTicks, shake, 0.10 * fade(progress), "migration_rumble");
    }

    private static double fade(double progress) {
        double clamped = Math.max(0.0, Math.min(1.0, progress));
        return Math.sin(clamped * Math.PI);
    }

    public record MigrationVisualEvent(
        String zoneId,
        double directionX,
        double directionZ,
        int durationTicks,
        int entityCount,
        long startedAtTick
    ) {
        public MigrationVisualEvent {
            zoneId = zoneId == null || zoneId.isBlank() ? "unknown" : zoneId.trim();
            durationTicks = Math.max(1, durationTicks);
            entityCount = Math.max(0, entityCount);
            startedAtTick = Math.max(0L, startedAtTick);
        }
    }

    public record MigrationVisualCommand(
        int dustPerEntityPerFiveTicks,
        double cameraShakeIntensity,
        double fogDensityDelta,
        String rumbleRecipeId
    ) {
        public MigrationVisualCommand {
            dustPerEntityPerFiveTicks = Math.max(0, dustPerEntityPerFiveTicks);
            cameraShakeIntensity = clamp01(cameraShakeIntensity);
            fogDensityDelta = clamp01(fogDensityDelta);
            rumbleRecipeId = rumbleRecipeId == null ? "" : rumbleRecipeId.trim();
        }

        public static MigrationVisualCommand none() {
            return new MigrationVisualCommand(0, 0.0, 0.0, "");
        }

        private static double clamp01(double value) {
            if (!Double.isFinite(value)) {
                return 0.0;
            }
            return Math.max(0.0, Math.min(1.0, value));
        }
    }
}
