package com.bong.client.npc;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NpcMoodIconTest {
    @Test
    void mood_icon_alpha_fade() {
        assertEquals(0, NpcMoodIcon.alphaAt(1_000L, 1_000L));
        assertTrue(NpcMoodIcon.alphaAt(1_000L, 1_150L) > 120);
        assertEquals(255, NpcMoodIcon.alphaAt(1_000L, 1_300L));
    }

    @Test
    void mood_transition_color_lerp() {
        int start = NpcMoodIcon.transitionColor("alert", "hostile", 1_000L, 1_000L);
        int middle = NpcMoodIcon.transitionColor("alert", "hostile", 1_000L, 1_100L);
        int end = NpcMoodIcon.transitionColor("alert", "hostile", 1_000L, 1_200L);

        assertNotEquals(start, middle);
        assertNotEquals(middle, end);
        assertEquals(end, NpcMoodIcon.transitionColor("hostile", "hostile", 1_000L, 1_000L));
    }

    @Test
    void mood_icon_texture_contract_is_stable() {
        assertEquals("bong-client:textures/gui/npc/mood_alert.png", NpcMoodIcon.texturePath("ALERT"));
        assertEquals(14, NpcMoodIcon.iconSize("hostile"));
    }
}
