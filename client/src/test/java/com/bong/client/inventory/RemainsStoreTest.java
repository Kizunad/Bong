package com.bong.client.inventory;

import com.bong.client.inventory.state.RemainsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-remains-suite P0 — {@link RemainsStore} 单测（照 {@code DroppedItemStoreTest} 的形状）。
 */
public class RemainsStoreTest {

    @AfterEach
    void tearDown() {
        RemainsStore.resetForTests();
    }

    private static RemainsStore.Entry entry(String id, double x, double y, double z) {
        return new RemainsStore.Entry(id, x, y, z, "minecraft:overworld", "遗骸", 3, 12L);
    }

    @Test
    void putSnapshotAndRemoveRoundtrip() {
        RemainsStore.putOrReplace(entry("uuid-a", 8.0, 66.0, 8.0));

        assertEquals(1, RemainsStore.snapshot().size());
        assertEquals("遗骸", RemainsStore.get("uuid-a").displayName());
        assertEquals(8.0, RemainsStore.get("uuid-a").worldPosX());
        assertEquals(3, RemainsStore.get("uuid-a").itemCount());
        assertEquals(12L, RemainsStore.get("uuid-a").boneCoins());

        RemainsStore.remove("uuid-a");

        assertNull(RemainsStore.get("uuid-a"));
        assertEquals(0, RemainsStore.snapshot().size());
    }

    @Test
    void removeMissingIdIsNoOp() {
        RemainsStore.putOrReplace(entry("kept", 8.0, 66.0, 8.0));

        RemainsStore.remove("missing");
        RemainsStore.remove(null);

        assertEquals(1, RemainsStore.snapshot().size(), "删除不存在 id 不应影响已有遗骸缓存");
        assertEquals("kept", RemainsStore.get("kept").remainsId());
    }

    @Test
    void nearestToReturnsClosestEntry() {
        RemainsStore.putOrReplace(entry("near", 2.0, 0.0, 2.0));
        RemainsStore.putOrReplace(entry("far", 8.0, 0.0, 8.0));

        RemainsStore.Entry nearest = RemainsStore.nearestTo(0.0, 0.0, 0.0);

        assertEquals("near", nearest.remainsId());
    }

    /**
     * 等距时后放入者胜（insertionOrder tie-breaker）——与 DroppedItemStore 同款语义，
     * 保证 marker 渲染目标与 G 键 pickup 目标一致，不随 HashMap 迭代顺序抖动。
     */
    @Test
    void nearestToUsesInsertionOrderAsTieBreaker() {
        RemainsStore.putOrReplace(entry("first", 3.0, 0.0, 4.0));  // 距原点 5
        RemainsStore.putOrReplace(entry("second", 4.0, 0.0, 3.0)); // 距原点 5，严格等距

        assertEquals("second", RemainsStore.nearestTo(0.0, 0.0, 0.0).remainsId(),
            "latest inserted entry should win when distances tie");
        for (int i = 0; i < 10; i++) {
            assertEquals("second", RemainsStore.nearestTo(0.0, 0.0, 0.0).remainsId());
        }
    }

    @Test
    void nearestToUsesLatestInsertionAcrossThreeWayTie() {
        RemainsStore.putOrReplace(entry("first", 3.0, 0.0, 4.0));
        RemainsStore.putOrReplace(entry("second", 4.0, 0.0, 3.0));
        RemainsStore.putOrReplace(entry("third", 0.0, 0.0, 5.0));

        assertEquals(
            "third",
            RemainsStore.nearestTo(0.0, 0.0, 0.0).remainsId(),
            "三具遗骸严格等距时应由最后插入者胜出"
        );
    }

    /** replace（同 id 重新 put）不应更新 insertionOrder——server 每次全量推送不得洗掉 latest 语义。 */
    @Test
    void putOrReplacePreservesInsertionOrderOnReplace() {
        RemainsStore.putOrReplace(entry("a", 1.0, 0.0, 0.0));
        RemainsStore.putOrReplace(entry("b", 0.0, 0.0, 1.0)); // 与 a 等距（1）

        assertEquals("b", RemainsStore.nearestTo(0.0, 0.0, 0.0).remainsId());

        RemainsStore.putOrReplace(entry("a", 0.707, 0.0, 0.707)); // 距离仍 1

        assertEquals("b", RemainsStore.nearestTo(0.0, 0.0, 0.0).remainsId(),
            "replace should not renew insertionOrder");
    }

    /** replaceAll 按 list 顺序分配 insertionOrder，list 尾 = latest。 */
    @Test
    void replaceAllAssignsOrderByListPosition() {
        RemainsStore.replaceAll(java.util.List.of(
            entry("head", 1.0, 0.0, 0.0),
            entry("tail", 0.0, 0.0, 1.0)
        ));

        assertEquals("tail", RemainsStore.nearestTo(0.0, 0.0, 0.0).remainsId(),
            "list-tail entry should win the tie (treated as latest)");
    }

    @Test
    void replaceAllDeduplicatesByRemainsIdUsingLastEntryValue() {
        RemainsStore.replaceAll(java.util.List.of(
            entry("dup", 1.0, 0.0, 0.0),
            new RemainsStore.Entry("dup", 9.0, 70.0, 9.0, "minecraft:overworld", "新遗骸", 5, 8L)
        ));

        assertEquals(1, RemainsStore.snapshot().size(), "重复 remains_id 应只保留一条缓存记录");
        RemainsStore.Entry entry = RemainsStore.get("dup");
        assertEquals("新遗骸", entry.displayName(), "重复 id 的后续条目应覆盖可见字段");
        assertEquals(5, entry.itemCount());
        assertEquals(8L, entry.boneCoins());
    }

    @Test
    void replaceAllWithEmptyListClearsStore() {
        RemainsStore.putOrReplace(entry("stale", 1.0, 0.0, 0.0));

        RemainsStore.replaceAll(java.util.List.of());

        assertTrue(RemainsStore.snapshot().isEmpty(),
            "空快照必须清空 store（遗骸全被搬空后世界里应消失）");
    }

    @Test
    void clearOnDisconnectDropsEverything() {
        RemainsStore.putOrReplace(entry("session-old", 1.0, 0.0, 0.0));

        RemainsStore.clearOnDisconnect();

        assertTrue(RemainsStore.snapshot().isEmpty(),
            "断线必须清空遗骸缓存，防止 reconnect 后 G 键命中上一 server 的幽灵遗骸");
        assertNull(RemainsStore.nearestTo(0.0, 0.0, 0.0));
    }

    @Test
    void nearestToOnEmptyStoreReturnsNull() {
        assertNull(RemainsStore.nearestTo(0.0, 0.0, 0.0),
            "空 store 应返回 null（G 键候选缺席，不参与优先级竞争）");
    }
}
