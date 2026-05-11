package com.bong.client.npc;

import java.util.Collection;
import java.util.Comparator;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class NpcMoodStore {
    private static final Map<Integer, NpcMoodState> MOOD_BY_ENTITY_ID = new ConcurrentHashMap<>();

    private NpcMoodStore() {
    }

    public static void upsert(NpcMoodState state) {
        if (state == null || state.entityId() < 0) {
            return;
        }
        MOOD_BY_ENTITY_ID.put(state.entityId(), state);
    }

    public static NpcMoodState get(int entityId) {
        return MOOD_BY_ENTITY_ID.get(entityId);
    }

    public static Collection<NpcMoodState> snapshot() {
        return MOOD_BY_ENTITY_ID.values().stream()
            .sorted(Comparator.comparingInt(NpcMoodState::entityId))
            .toList();
    }

    public static void clearAll() {
        MOOD_BY_ENTITY_ID.clear();
    }
}
