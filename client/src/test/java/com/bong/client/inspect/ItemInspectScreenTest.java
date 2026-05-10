package com.bong.client.inspect;

import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ItemInspectScreenTest {
    @Test
    void itemInspectDetailLinesIncludeCoreAndForgeFields() {
        InventoryItem item = InventoryItem.createFullWithForgeMeta(
            42L,
            "iron_sword",
            "寒铁剑",
            1,
            3,
            2.5,
            "rare",
            "剑身有冷纹。",
            1,
            0.82,
            0.76,
            "",
            "",
            0,
            0.7,
            "cold",
            List.of("寒气"),
            2
        );

        List<String> lines = ItemInspectScreen.detailLines(item);

        assertTrue(lines.stream().anyMatch(line -> line.contains("名称: 寒铁剑")));
        assertTrue(lines.stream().anyMatch(line -> line.contains("品质: 82%")));
        assertTrue(lines.stream().anyMatch(line -> line.contains("保质期: 76%")));
        assertTrue(lines.stream().anyMatch(line -> line.contains("法器: 灵核 T2")));
        assertTrue(lines.stream().anyMatch(line -> line.contains("铭文槽: 空")));
    }

    @Test
    void itemInspectDetailLinesShowAppliedForgeInscription() {
        InventoryItem item = InventoryItem.createFullWithForgeMeta(
            42L,
            "iron_sword",
            "寒铁剑",
            1,
            3,
            2.5,
            "rare",
            "剑身有冷纹。",
            1,
            0.82,
            0.76,
            "",
            "sharp_v0",
            0,
            0.7,
            "cold",
            List.of("寒气"),
            2
        );

        List<String> lines = ItemInspectScreen.detailLines(item);

        assertTrue(lines.stream().anyMatch(line -> line.contains("铭文槽: sharp_v0")));
    }

    @Test
    void itemInspectOpensOnLongPress() {
        InventoryItem item = InventoryItem.simple("ning_mai_cao", "凝脉草");
        ItemInspectLongPressTracker tracker = new ItemInspectLongPressTracker();

        tracker.start(item, 10.0, 10.0, 1_000L);

        assertNull(tracker.consumeReady(1_999L));
        assertSame(item, tracker.consumeReady(2_000L));
        assertNull(tracker.consumeReady(2_100L));
    }
}
