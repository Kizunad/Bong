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
    void artifactPageRendersGroovesAndResonancePreview() {
        InventoryItem item = InventoryItem.createFullWithForgeMeta(
            42L,
            "bone_sword",
            "骨剑",
            1,
            2,
            0.9,
            "common",
            "骨质短剑。",
            1,
            0.8,
            1.0,
            "",
            "",
            0,
            0.9,
            "solid",
            List.of("artifact_state:{\"meridian\":{\"grooves\":[{\"depth\":20.0,\"depth_cap\":60.0,\"crack_severity\":0.2}],\"total_depth\":20.0,\"depth_cap\":60.0,\"quality_tier\":1,\"overload_cracks\":2},\"color\":{\"practice_log\":{\"weights\":{\"Solid\":10.0}},\"main\":\"Solid\",\"secondary\":null,\"is_chaotic\":false}}"),
            1
        );

        List<String> lines = ItemInspectScreen.detailLines(item);

        assertTrue(
            lines.stream().anyMatch(line -> line.contains("当前附着: 无")),
            "detailLines should contain '当前附着: 无', actual lines: " + lines
        );
        assertTrue(
            lines.stream().anyMatch(line -> line.contains("铭纹: 1槽")),
            "detailLines should contain '铭纹: 1槽', actual lines: " + lines
        );
        assertTrue(
            lines.stream().anyMatch(line -> line.contains("共鸣提示: 27%")),
            "detailLines should contain '共鸣提示: 27%', actual lines: " + lines
        );
        assertTrue(
            lines.stream().anyMatch(line -> line.contains("龟裂: 裂纹")),
            "detailLines should contain '龟裂: 裂纹', actual lines: " + lines
        );
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
