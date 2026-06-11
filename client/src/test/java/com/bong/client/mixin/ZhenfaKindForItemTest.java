package com.bong.client.mixin;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.network.ClientRequestProtocol;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

class ZhenfaKindForItemTest {
    @Test
    void gatherArrayBaseMapsToLingju() {
        assertEquals(
            ClientRequestProtocol.ZhenfaKind.LINGJU,
            MixinClientPlayerInteractionManagerAlchemy.bong$zhenfaKindForItem(item("gather_array_base")),
            "gather_array_base 必须走 lingju 放置协议，否则 workbench 聚灵阵基座仍是僵尸物品"
        );
    }

    @Test
    void nonZhenfaItemDoesNotTriggerLingjuPlacement() {
        assertNull(
            MixinClientPlayerInteractionManagerAlchemy.bong$zhenfaKindForItem(item("qi_scatter_bead")),
            "P1 只接 gather_array_base；qi_scatter_bead 留给 P2 use handler，不能误触发 Lingju"
        );
    }

    private static InventoryItem item(String itemId) {
        return InventoryItem.create(itemId, itemId, 1, 1, 0.1, "common", "");
    }
}
