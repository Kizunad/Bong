package com.bong.client.visual.realm_vision;

public final class RealmVisionPlanner {
    private RealmVisionPlanner() {
    }

    public static RealmVisionCommand plan(RealmVisionState state, long tick) {
        if (state == null || state.isEmpty()) {
            return null;
        }
        long tickElapsed = tick > state.startedAtTick() ? tick - state.startedAtTick() : 0L;
        int elapsed = Math.max(state.elapsedTicks(), (int) Math.min(Integer.MAX_VALUE, tickElapsed));
        return RealmVisionInterpolator.interpolate(
            state.previous(),
            state.current(),
            state.transitionTicks(),
            elapsed
        );
    }

    public static RealmVisionCommand clampToRenderDistance(
        RealmVisionCommand command,
        int renderDistanceChunks
    ) {
        if (command == null || renderDistanceChunks <= 0) {
            return command;
        }
        double limit = renderDistanceChunks * 16.0 - 4.0;
        double fogStart = Math.min(command.fogStart(), limit);
        double fogEnd = Math.min(command.fogEnd(), limit);
        return new RealmVisionCommand(
            fogStart,
            fogEnd,
            command.fogColorRgb(),
            command.fogShape(),
            command.vignetteAlpha(),
            command.tintColorArgb(),
            command.particleDensity(),
            command.postFxSharpen()
        );
    }
}
