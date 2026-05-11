package com.bong.client.npc;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NpcDialogueBubbleRendererTest {
    @AfterEach
    void reset() {
        NpcDialogueBubbleRenderer.clearForTests();
    }

    @Test
    void bubble_alpha_distance_decay() {
        assertEquals(255, NpcDialogueBubbleRenderer.alphaForDistance(10.0));
        assertEquals(128, NpcDialogueBubbleRenderer.alphaForDistance(20.0));
        assertEquals(0, NpcDialogueBubbleRenderer.alphaForDistance(25.0));
    }

    @Test
    void bubble_hidden_during_dialogue_screen() {
        NpcMetadata metadata = new NpcMetadata(
            42,
            "rogue",
            "凝脉",
            null,
            null,
            0,
            "散修·凝脉",
            "正值壮年",
            "道友...",
            null
        );

        assertTrue(NpcDialogueBubbleRenderer.hiddenDuringDialogueScreen(new NpcDialogueScreen(metadata)));
    }

    @Test
    void bubble_wraps_to_three_lines() {
        List<String> lines = NpcDialogueBubbleRenderer.wrapLines(
            "道友上次给的灵草味道不对这次若还是如此就别怪我翻脸",
            48,
            text -> text.length() * 6
        );

        assertFalse(lines.isEmpty());
        assertTrue(lines.size() <= 3);
        assertTrue(lines.stream().allMatch(line -> line.length() * 6 <= 48 || line.endsWith("...")));
    }

    @Test
    void bubble_store_keeps_one_active_bubble_per_npc() {
        NpcDialogueBubbleRenderer.show(new NpcDialogueBubbleRenderer.Bubble(7, "第一句", "greeting", "rogue", 3_000L, 1_000L));
        NpcDialogueBubbleRenderer.show(new NpcDialogueBubbleRenderer.Bubble(7, "第二句", "warning", "rogue", 3_000L, 1_100L));

        List<NpcDialogueBubbleRenderer.Bubble> bubbles = NpcDialogueBubbleRenderer.snapshot(1_200L);

        assertEquals(1, bubbles.size());
        assertEquals("第二句", bubbles.get(0).text());
    }
}
