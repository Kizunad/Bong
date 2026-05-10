package com.bong.client.atmosphere;

import net.minecraft.util.math.Vec3d;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class AshFootprintTracker {
    private static final double MIN_STEP_DISTANCE_SQ = 0.36;
    private static final long MIN_STEP_INTERVAL_TICKS = 6L;

    private final Map<Long, StepSnapshot> lastStepByEntity = new LinkedHashMap<>();

    public List<FootprintCommand> onEntityStep(long entityId, Vec3d pos, long worldTick, ZoneAtmosphereCommand atmosphere) {
        if (entityId < 0 || pos == null || atmosphere == null || !atmosphere.deadZoneVisual()) {
            return List.of();
        }
        StepSnapshot previous = lastStepByEntity.get(entityId);
        if (previous != null
            && previous.pos().squaredDistanceTo(pos) < MIN_STEP_DISTANCE_SQ
            && worldTick - previous.worldTick() < MIN_STEP_INTERVAL_TICKS) {
            return List.of();
        }
        lastStepByEntity.put(entityId, new StepSnapshot(pos, worldTick));
        return List.of(
            new FootprintCommand("ash_burst", 0x808080, 2, 20),
            new FootprintCommand("ash_footprint_decal", 0x555555, 1, 600)
        );
    }

    public void clear() {
        lastStepByEntity.clear();
    }

    public record FootprintCommand(String kind, int tintRgb, int count, int lifetimeTicks) {
        public FootprintCommand {
            kind = ZoneAtmosphereProfile.normalizeId(kind, "ash_burst");
            tintRgb &= 0x00FFFFFF;
            count = Math.max(0, count);
            lifetimeTicks = Math.max(1, lifetimeTicks);
        }
    }

    private record StepSnapshot(Vec3d pos, long worldTick) {
    }
}
