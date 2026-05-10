package com.bong.client.npc;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NpcNametagRendererTest {
    @Test
    void nametagColorTracksReputation() {
        assertEquals(0xE05A47, NpcNametagRenderer.colorByReputation(-31));
        assertEquals(0xC8C8C8, NpcNametagRenderer.colorByReputation(-30));
        assertEquals(0xC8C8C8, NpcNametagRenderer.colorByReputation(0));
        assertEquals(0xC8C8C8, NpcNametagRenderer.colorByReputation(50));
        assertEquals(0x5DD17A, NpcNametagRenderer.colorByReputation(51));
    }

    @Test
    void distanceLabelFallsBackToIconThenHides() {
        NpcMetadata metadata = new NpcMetadata(
            42,
            "rogue",
            "凝脉",
            null,
            null,
            0,
            "散修·凝脉",
            "正值壮年",
            "道友，可有灵草出让？",
            "真元流转平稳"
        );

        assertEquals("[散修·凝脉]", NpcNametagRenderer.labelForDistance(metadata, 19.0, "凝脉"));
        assertEquals("散", NpcNametagRenderer.labelForDistance(metadata, 25.0, "凝脉"));
        assertEquals("", NpcNametagRenderer.labelForDistance(metadata, 40.0, "凝脉"));
        assertEquals("[散修·凝脉]", NpcNametagRenderer.labelForDistance(metadata, 19.0, "引气"));
        assertTrue(NpcNametagRenderer.labelForDistance(metadata, 19.0, "Awaken").startsWith("⚠ "));
        assertEquals("[散修·凝脉]", NpcNametagRenderer.labelForDistance(metadata, 19.0, "Induce"));
    }
}
