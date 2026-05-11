package com.bong.client.npc;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class NpcMoodStoreTest {
    @AfterEach
    void tearDown() {
        NpcMoodStore.clearAll();
    }

    @Test
    void upsert_ignores_older_mood_packets() {
        NpcMoodStore.upsert(new NpcMoodState(7, "hostile", 0.9, "洞虚", "杀意浮动", 2_000L));
        NpcMoodStore.upsert(new NpcMoodState(7, "neutral", 0.1, "醒灵", "路过", 1_000L));

        NpcMoodState state = NpcMoodStore.get(7);
        assertEquals("hostile", state.mood());
        assertEquals(2_000L, state.updatedAtMillis());
    }

    @Test
    void upsert_accepts_equal_or_newer_mood_packets() {
        NpcMoodStore.upsert(new NpcMoodState(7, "alert", 0.4, null, null, 1_000L));
        NpcMoodStore.upsert(new NpcMoodState(7, "fearful", 0.2, null, null, 1_000L));
        NpcMoodStore.upsert(new NpcMoodState(7, "hostile", 0.8, null, null, 1_001L));

        NpcMoodState state = NpcMoodStore.get(7);
        assertEquals("hostile", state.mood());
        assertEquals(1_001L, state.updatedAtMillis());
    }
}
