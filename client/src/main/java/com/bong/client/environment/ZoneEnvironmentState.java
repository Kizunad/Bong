package com.bong.client.environment;

import java.util.List;
import java.util.Objects;

public record ZoneEnvironmentState(
    int version,
    String zoneId,
    List<EnvironmentEffect> effects,
    long generation
) {
    public ZoneEnvironmentState {
        zoneId = zoneId == null ? "" : zoneId.trim();
        effects = List.copyOf(effects == null ? List.of() : effects);
        generation = Math.max(0L, generation);
    }

    public boolean valid() {
        return version == 1 && !zoneId.isBlank();
    }

    public static ZoneEnvironmentState empty(String zoneId) {
        return new ZoneEnvironmentState(1, Objects.requireNonNullElse(zoneId, ""), List.of(), 0L);
    }
}
