package com.bong.client.spirittreasure;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;

public final class SpiritTreasureStateStore {
    private static volatile List<SpiritTreasureState> snapshot = List.of();
    private static volatile long updatedAtMs = 0L;

    private SpiritTreasureStateStore() {
    }

    public static synchronized void replace(List<SpiritTreasureState> treasures, long nowMs) {
        snapshot = List.copyOf(treasures == null ? List.of() : treasures);
        updatedAtMs = Math.max(0L, nowMs);
    }

    public static synchronized List<SpiritTreasureState> snapshot() {
        return new ArrayList<>(snapshot);
    }

    public static synchronized Optional<SpiritTreasureState> byTemplateId(String templateId) {
        if (templateId == null || templateId.isBlank()) {
            return Optional.empty();
        }
        return snapshot.stream()
            .filter(treasure -> templateId.equals(treasure.templateId()))
            .findFirst();
    }

    public static synchronized long updatedAtMs() {
        return updatedAtMs;
    }

    public static synchronized void clear() {
        snapshot = List.of();
        updatedAtMs = 0L;
    }

    public static void resetForTests() {
        clear();
    }
}
