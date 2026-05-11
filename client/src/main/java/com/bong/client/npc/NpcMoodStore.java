package com.bong.client.npc;

import java.util.Collection;
import java.util.ArrayList;
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
        MOOD_BY_ENTITY_ID.compute(state.entityId(), (entityId, current) ->
            current == null || state.updatedAtMillis() >= current.updatedAtMillis() ? state : current
        );
    }

    public static NpcMoodState get(int entityId) {
        return MOOD_BY_ENTITY_ID.get(entityId);
    }

    /**
     * Returns an unsorted shallow copy; render callers should impose ordering only when needed.
     */
    public static Collection<NpcMoodState> snapshot() {
        return new ArrayList<>(MOOD_BY_ENTITY_ID.values());
    }

    public static void clearAll() {
        MOOD_BY_ENTITY_ID.clear();
    }
}
