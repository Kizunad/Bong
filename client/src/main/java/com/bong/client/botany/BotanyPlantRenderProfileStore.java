package com.bong.client.botany;

import java.util.Collection;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;

public final class BotanyPlantRenderProfileStore {
    private static volatile Map<String, BotanyPlantRenderProfile> profiles = Map.of();

    private BotanyPlantRenderProfileStore() {}

    public static Optional<BotanyPlantRenderProfile> get(String plantId) {
        if (plantId == null || plantId.isBlank()) {
            return Optional.empty();
        }
        return Optional.ofNullable(profiles.get(plantId.trim()));
    }

    public static Map<String, BotanyPlantRenderProfile> snapshot() {
        return profiles;
    }

    public static void replaceAll(Collection<BotanyPlantRenderProfile> nextProfiles) {
        if (nextProfiles == null || nextProfiles.isEmpty()) {
            profiles = Map.of();
            return;
        }
        ConcurrentHashMap<String, BotanyPlantRenderProfile> next = new ConcurrentHashMap<>();
        for (BotanyPlantRenderProfile profile : nextProfiles) {
            if (profile != null && !profile.plantId().isBlank()) {
                next.put(profile.plantId(), profile);
            }
        }
        profiles = Map.copyOf(next);
    }

    public static void clearOnDisconnect() {
        profiles = Map.of();
    }
}
