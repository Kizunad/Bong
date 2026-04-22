package com.bong.client.combat;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class TreasureEquippedStore {
    private static final Map<String, EquippedTreasure> snapshots = new ConcurrentHashMap<>();

    private TreasureEquippedStore() {
    }

    public static EquippedTreasure get(String slot) {
        return snapshots.get(slot);
    }

    public static void putOrClear(String slot, EquippedTreasure treasure) {
        if (treasure == null) {
            snapshots.remove(slot);
        } else {
            snapshots.put(slot, treasure);
        }
    }

    public static void resetForTests() {
        snapshots.clear();
    }

    public static void clearOnDisconnect() {
        snapshots.clear();
    }
}
