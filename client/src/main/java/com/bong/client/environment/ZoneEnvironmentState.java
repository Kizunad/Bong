package com.bong.client.environment;

import java.util.List;
import java.util.Objects;

public record ZoneEnvironmentState(
    int version,
    String dimension,
    String zoneId,
    List<EnvironmentEffect> effects,
    long generation
) {
    public static final String OVERWORLD_DIMENSION = "minecraft:overworld";

    public ZoneEnvironmentState(int version, String zoneId, List<EnvironmentEffect> effects, long generation) {
        this(version, OVERWORLD_DIMENSION, zoneId, effects, generation);
    }

    public ZoneEnvironmentState {
        dimension = dimension == null || dimension.isBlank() ? OVERWORLD_DIMENSION : dimension.trim();
        zoneId = zoneId == null ? "" : zoneId.trim();
        effects = List.copyOf(effects == null ? List.of() : effects);
        generation = Math.max(0L, generation);
    }

    public boolean valid() {
        return version == 1 && !dimension.isBlank() && !zoneId.isBlank();
    }

    public boolean matchesDimension(String currentDimension) {
        return currentDimension == null || currentDimension.isBlank() || dimension.equals(currentDimension.trim());
    }

    public static ZoneEnvironmentState empty(String zoneId) {
        return new ZoneEnvironmentState(1, OVERWORLD_DIMENSION, Objects.requireNonNullElse(zoneId, ""), List.of(), 0L);
    }
}
