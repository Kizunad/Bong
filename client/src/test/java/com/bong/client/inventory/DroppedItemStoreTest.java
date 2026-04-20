package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

public class DroppedItemStoreTest {

    @AfterEach
    void tearDown() {
        DroppedItemStore.resetForTests();
    }

    @Test
    void putSnapshotAndRemoveRoundtrip() {
        InventoryItem item = InventoryItem.createFull(
            1004L,
            "starter_talisman",
            "启程护符",
            1,
            1,
            0.2,
            "common",
            "fixture",
            1,
            0.5,
            1.0
        );

        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            8.0,
            66.0,
            8.0,
            item
        ));

        assertEquals(1, DroppedItemStore.snapshot().size());
        assertEquals("main_pack", DroppedItemStore.get(1004L).sourceContainerId());
        assertEquals(8.0, DroppedItemStore.get(1004L).worldPosX());

        DroppedItemStore.remove(1004L);

        assertNull(DroppedItemStore.get(1004L));
        assertEquals(0, DroppedItemStore.snapshot().size());
    }

    @Test
    void nearestToReturnsClosestEntry() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            2.0,
            0.0,
            2.0,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1005L,
            "main_pack",
            0,
            1,
            8.0,
            0.0,
            8.0,
            InventoryItem.simple("old_coin", "旧铜钱")
        ));

        DroppedItemStore.Entry nearest = DroppedItemStore.nearestTo(0.0, 0.0, 0.0);

        assertEquals(1004L, nearest.instanceId());
    }

    /**
     * 两个物品等距时，后放入的 insertionOrder 大者胜出——marker 渲染目标与 G 键 pickup 目标因此保持一致。
     * 若不做 tie-breaker，HashMap 迭代顺序决定输赢，两次调用可能返回不同 entry。
     */
    @Test
    void nearestToUsesInsertionOrderAsTieBreaker() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            2001L,
            "main_pack",
            0,
            0,
            3.0,
            0.0,
            4.0,  // 距离原点 5
            InventoryItem.simple("stone", "石")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            2002L,
            "main_pack",
            0,
            1,
            4.0,
            0.0,
            3.0,  // 距离原点 5，与 2001 严格等距
            InventoryItem.simple("wood", "木")
        ));

        DroppedItemStore.Entry nearest = DroppedItemStore.nearestTo(0.0, 0.0, 0.0);

        assertEquals(2002L, nearest.instanceId(), "latest inserted entry should win when distances tie");

        // 连续 10 次调用都应稳定返回同一个（HashMap 无 tie-breaker 时可能抖动）。
        for (int i = 0; i < 10; i++) {
            assertEquals(2002L, DroppedItemStore.nearestTo(0.0, 0.0, 0.0).instanceId());
        }
    }

    /**
     * replace（相同 instanceId 重新 put）不应更新 insertionOrder——
     * 否则 server 每次推 snapshot 都会把所有物品 order 洗一遍，latest 语义失效。
     */
    @Test
    void putOrReplacePreservesInsertionOrderOnReplace() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            3001L, "main_pack", 0, 0,
            1.0, 0.0, 0.0,
            InventoryItem.simple("a", "A")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            3002L, "main_pack", 0, 1,
            0.0, 0.0, 1.0,  // 与 3001 等距（距离 1）
            InventoryItem.simple("b", "B")
        ));

        assertEquals(3002L, DroppedItemStore.nearestTo(0.0, 0.0, 0.0).instanceId());

        // 重新 put 3001（同 id，位置挪了一点点但仍等距），order 应保持原值不变。
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            3001L, "main_pack", 0, 0,
            0.707, 0.0, 0.707,  // 距离仍 1
            InventoryItem.simple("a", "A'")
        ));

        assertEquals(3002L, DroppedItemStore.nearestTo(0.0, 0.0, 0.0).instanceId(),
            "replace should not renew insertionOrder");
    }

    /**
     * replaceAll（server snapshot 全量）按 list 顺序分配 insertionOrder，
     * 最后一个才是 latest——这与 server "append 新 drop 到 list 尾" 的约定对齐。
     */
    @Test
    void replaceAllAssignsOrderByListPosition() {
        java.util.List<DroppedItemStore.Entry> list = java.util.List.of(
            new DroppedItemStore.Entry(4001L, "main_pack", 0, 0,
                1.0, 0.0, 0.0, InventoryItem.simple("first", "first")),
            new DroppedItemStore.Entry(4002L, "main_pack", 0, 1,
                0.0, 0.0, 1.0, InventoryItem.simple("last", "last"))
        );

        DroppedItemStore.replaceAll(list);

        assertEquals(4002L, DroppedItemStore.nearestTo(0.0, 0.0, 0.0).instanceId(),
            "list-tail entry should win the tie (treated as latest)");
    }

    @Test
    void removeAlsoDropsInsertionOrder() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            5001L, "main_pack", 0, 0,
            1.0, 0.0, 0.0, InventoryItem.simple("a", "A")
        ));
        DroppedItemStore.remove(5001L);
        // 重新 put 同 id，应拿到新 order（>= 2：之前是 1，remove 清了，这次 counter 又 +1）。
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            5001L, "main_pack", 0, 0,
            1.0, 0.0, 0.0, InventoryItem.simple("a", "A")
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            5002L, "main_pack", 0, 1,
            0.0, 0.0, 1.0, InventoryItem.simple("b", "B")
        ));

        // 5002 是最后 put 的，等距时 5002 应胜。
        assertEquals(5002L, DroppedItemStore.nearestTo(0.0, 0.0, 0.0).instanceId());
    }
}
