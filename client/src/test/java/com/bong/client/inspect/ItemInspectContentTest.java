package com.bong.client.inspect;

import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;
import java.util.List;
import static org.junit.jupiter.api.Assertions.*;

class ItemInspectContentTest {
    @Test
    void ordinaryItemUsesServerMetadataAndOmitsUnavailableProperties() {
        var item = InventoryItem.createFull(42L, "pickaxe_iron", "铁镐", 1, 2, 1.8,
            "common", "贫铁矿工镐，能稳稳敲开大多数凡矿。", 1, 0, 0.76);
        var rows = ItemInspectContent.detailRows(item);
        assertTrue(rows.contains(new ItemInspectContent.Detail("名称", item.displayName())));
        assertTrue(rows.contains(new ItemInspectContent.Detail("重量", "1.8 kg")));
        assertTrue(rows.contains(new ItemInspectContent.Detail("占格", "1 x 2")));
        assertTrue(rows.contains(new ItemInspectContent.Detail("耐久", "76%")),
            "durability 必须显示为耐久，不能冒充保质期");
        assertFalse(rows.stream().anyMatch(row -> List.of("充能次数", "保质期", "灵材", "灵核").contains(row.label())),
            "缺少的属性不得通过物品 id 猜测或填占位值");
    }

    @Test
    void chargesAreOptionalAndIndependentOfForgeTierIncludingDepletedState() {
        for (Integer charges : new Integer[]{null, 0, 2}) {
            var item = InventoryItem.createFullWithVisualMeta(42L, "ancient_relic", "遗物",
                1, 1, 1, "ancient", "", 1, 1, 1, charges,
                "", "sharp_v0", 0, 0.7, "cold", List.of("寒气"), 4, List.of());
            var rows = ItemInspectContent.detailRows(item);
            var chargeRows = rows.stream().filter(row -> row.label().equals("充能次数")).toList();
            assertEquals(charges == null ? List.of() : List.of(new ItemInspectContent.Detail("充能次数", charges.toString())),
                chargeRows, "未配置充能应隐藏；已耗尽的 0 次必须保留，且不能误读灵核等级");
            assertTrue(rows.contains(new ItemInspectContent.Detail("灵核", "T4")));
            assertTrue(rows.contains(new ItemInspectContent.Detail("铭文", "sharp_v0")));
            assertTrue(rows.contains(new ItemInspectContent.Detail("当前附着", "寒气")));
        }
    }

    @Test
    void artifactMetadataIsPresentedAsPropertiesWithoutExposingRawPayload() {
        var item = InventoryItem.createFullWithForgeMeta(42L, "bone_sword", "骨剑", 1, 2, 0.9,
            "common", "骨质短剑。", 1, 0.8, 1, "", "", 0, 0.9, "solid",
            List.of("artifact_state:{\"meridian\":{\"grooves\":[{\"depth\":20.0,\"depth_cap\":60.0,\"crack_severity\":0.2}],\"total_depth\":20.0,\"depth_cap\":60.0,\"quality_tier\":1,\"overload_cracks\":2},\"color\":{\"practice_log\":{\"weights\":{\"Solid\":10.0}},\"main\":\"Solid\",\"secondary\":null,\"is_chaotic\":false}}"),
            1);
        var rows = ItemInspectContent.detailRows(item);
        assertTrue(rows.contains(new ItemInspectContent.Detail("铭纹", "1槽")));
        assertTrue(rows.contains(new ItemInspectContent.Detail("共鸣提示", "27%")));
        assertTrue(rows.contains(new ItemInspectContent.Detail("龟裂", "裂纹")));
        assertFalse(rows.stream().anyMatch(row -> row.label().equals("当前附着")));
    }
}
