package com.bong.client.botany;

import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class BotanyPlantStageVisualStore {
    private static final Map<String, BotanyPlantStageVisual> VISUALS = new ConcurrentHashMap<>();

    private BotanyPlantStageVisualStore() {
    }

    public static void upsert(BotanyPlantStageVisual visual) {
        if (visual == null || visual.key().isBlank() || visual.plantId().isBlank()) {
            return;
        }
        VISUALS.put(visual.key(), visual);
    }

    public static List<BotanyPlantStageVisual> snapshot() {
        return List.copyOf(VISUALS.values());
    }

    public static void clearExpired(long worldTime) {
        VISUALS.entrySet().removeIf(entry -> entry.getValue().expired(worldTime));
    }

    public static void clear() {
        VISUALS.clear();
    }
}
