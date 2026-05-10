package com.bong.client.npc;

import java.util.Collection;
import java.util.Comparator;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class NpcMetadataStore {
    private static final Map<Integer, NpcMetadata> METADATA_BY_ENTITY_ID = new ConcurrentHashMap<>();

    private NpcMetadataStore() {
    }

    public static void upsert(NpcMetadata metadata) {
        if (metadata == null) {
            return;
        }
        METADATA_BY_ENTITY_ID.put(metadata.entityId(), metadata);
    }

    public static NpcMetadata get(int entityId) {
        return METADATA_BY_ENTITY_ID.get(entityId);
    }

    public static Collection<NpcMetadata> snapshot() {
        return METADATA_BY_ENTITY_ID.values().stream()
            .sorted(Comparator.comparingInt(NpcMetadata::entityId))
            .toList();
    }

    public static void remove(int entityId) {
        METADATA_BY_ENTITY_ID.remove(entityId);
    }

    public static void clearAll() {
        METADATA_BY_ENTITY_ID.clear();
    }
}
