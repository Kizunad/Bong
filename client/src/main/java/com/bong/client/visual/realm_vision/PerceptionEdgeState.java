package com.bong.client.visual.realm_vision;

import java.util.List;

public record PerceptionEdgeState(List<SenseEntry> entries, long generation) {
    public PerceptionEdgeState {
        entries = entries == null ? List.of() : List.copyOf(entries);
        generation = Math.max(0L, generation);
    }

    public static PerceptionEdgeState empty() {
        return new PerceptionEdgeState(List.of(), 0L);
    }

    public boolean isEmpty() {
        return entries.isEmpty();
    }

    public record SenseEntry(SenseKind kind, double x, double y, double z, double intensity) {
        public SenseEntry {
            kind = kind == null ? SenseKind.LIVING_QI : kind;
            intensity = clamp01(intensity);
        }
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
