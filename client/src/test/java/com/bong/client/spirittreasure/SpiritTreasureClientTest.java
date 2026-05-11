package com.bong.client.spirittreasure;

import com.bong.client.network.ServerDataRouter;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class SpiritTreasureClientTest {
    @BeforeEach
    void resetStores() {
        SpiritTreasureStateStore.resetForTests();
        SpiritTreasureDialogueStore.resetForTests();
    }

    @Test
    void routesSpiritTreasureStateIntoStore() {
        String json = """
            {
              "v": 1,
              "type": "spirit_treasure_state",
              "treasures": [
                {
                  "template_id": "spirit_treasure_jizhaojing",
                  "display_name": "寂照镜",
                  "instance_id": 88,
                  "equipped": true,
                  "passive_active": true,
                  "affinity": 0.5,
                  "sleeping": false,
                  "source_sect": "清风宗",
                  "icon_texture": "bong-client:textures/gui/items/spirit_treasure_jizhaojing.png",
                  "passive_effects": [
                    {
                      "kind": "SpiritTreasurePerception",
                      "value": 0.3,
                      "description": "感知范围 +30%"
                    }
                  ]
                }
              ]
            }
            """;

        ServerDataRouter.RouteResult result = route(json);

        assertTrue(result.isHandled());
        List<SpiritTreasureState> snapshot = SpiritTreasureStateStore.snapshot();
        assertEquals(1, snapshot.size());
        assertEquals("spirit_treasure_jizhaojing", snapshot.get(0).templateId());
        assertEquals("寂照镜", snapshot.get(0).displayName());
        assertEquals(1, snapshot.get(0).passiveEffects().size());
    }

    @Test
    void routesSpiritTreasureDialogueIntoStore() {
        String json = """
            {
              "v": 1,
              "type": "spirit_treasure_dialogue",
              "dialogue": {
                "v": 1,
                "request_id": "spirit_treasure:7:840",
                "character_id": "offline:Azure",
                "treasure_id": "spirit_treasure_jizhaojing",
                "text": "镜中不见你，只见你脚下灵脉往西北偏。",
                "tone": "curious",
                "affinity_delta": 0.03
              },
              "display_name": "寂照镜",
              "zone": "spawn"
            }
            """;

        ServerDataRouter.RouteResult result = route(json);

        assertTrue(result.isHandled());
        List<SpiritTreasureDialogue> dialogues =
            SpiritTreasureDialogueStore.recentFor("spirit_treasure_jizhaojing");
        assertEquals(1, dialogues.size());
        assertEquals("镜中不见你，只见你脚下灵脉往西北偏。", dialogues.get(0).text());
        assertEquals("curious", dialogues.get(0).tone());
    }

    @Test
    void mirrorAffinityPaletteGetsBrighterWithAffinity() {
        int low = JiZhaoJingMirrorRenderer.colorForAffinity(0.0);
        int high = JiZhaoJingMirrorRenderer.colorForAffinity(1.0);

        assertTrue((high & 0x00FF00) > (low & 0x00FF00));
    }

    private static ServerDataRouter.RouteResult route(String json) {
        return ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);
    }
}
