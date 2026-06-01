package com.bong.client.npc;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

/**
 * {@link NpcLodStore} CRUD + 线程安全 + 生命周期测试（plan-offscreen-war-v1 §P7 PR-11）。
 */
public class NpcLodStoreTest {

    @AfterEach
    void clearStore() {
        NpcLodStore.clearAll();
    }

    // ── upsert / get ──────────────────────────────────────────────────────────

    @Test
    void upsertAndGetRoundtrip() {
        NpcLodSnapshot snap = new NpcLodSnapshot(10, "rogue", "引气", 0.8f, 100.0, 64.0, 200.0);
        NpcLodStore.upsert(snap);

        NpcLodSnapshot got = NpcLodStore.get(10);
        assertNotNull(got, "期望 upsert 后 get(10) 返回非 null，但得到 null");
        assertEquals(10, got.entityId(), "期望 entityId=10，实际=" + got.entityId());
        assertEquals("rogue", got.archetype(), "期望 archetype=rogue，实际=" + got.archetype());
    }

    @Test
    void getMissingEntityIdReturnsNull() {
        NpcLodSnapshot got = NpcLodStore.get(9999);
        assertNull(got, "期望查询未存储的 entityId=9999 返回 null，实际=" + got);
    }

    @Test
    void upsertNullIsNoop() {
        NpcLodStore.upsert(null);
        assertEquals(0, NpcLodStore.size(),
            "期望 upsert(null) 不改变 store 大小，实际=" + NpcLodStore.size());
    }

    @Test
    void upsertOverwritesExistingEntry() {
        NpcLodStore.upsert(new NpcLodSnapshot(5, "zombie", "醒灵", 0.9f, 0, 0, 0));
        NpcLodStore.upsert(new NpcLodSnapshot(5, "beast", "凝脉", 0.3f, 10, 0, 10));

        NpcLodSnapshot got = NpcLodStore.get(5);
        assertNotNull(got);
        assertEquals("beast", got.archetype(),
            "期望第二次 upsert 覆盖 archetype 为 'beast'，实际=" + got.archetype());
        assertEquals("凝脉", got.realm(),
            "期望第二次 upsert 覆盖 realm 为 '凝脉'，实际=" + got.realm());
    }

    // ── remove ────────────────────────────────────────────────────────────────

    @Test
    void removedEntityNotFound() {
        NpcLodStore.upsert(new NpcLodSnapshot(7, "commoner", "醒灵", 1.0f, 0, 0, 0));
        NpcLodStore.remove(7);

        assertNull(NpcLodStore.get(7),
            "期望 remove(7) 后 get(7) 返回 null，但仍有数据");
    }

    @Test
    void removeMissingIdIsNoop() {
        NpcLodStore.upsert(new NpcLodSnapshot(1, "rogue", "引气", 1.0f, 0, 0, 0));
        NpcLodStore.remove(999); // 不存在的 id

        assertEquals(1, NpcLodStore.size(),
            "期望 remove 不存在的 id 后 store 大小不变（仍为 1），实际=" + NpcLodStore.size());
    }

    // ── clearAll ──────────────────────────────────────────────────────────────

    @Test
    void clearAllEmptiesStore() {
        NpcLodStore.upsert(new NpcLodSnapshot(1, "zombie", "醒灵", 0.5f, 0, 0, 0));
        NpcLodStore.upsert(new NpcLodSnapshot(2, "rogue", "引气", 0.7f, 1, 0, 1));

        NpcLodStore.clearAll();

        assertEquals(0, NpcLodStore.size(),
            "期望 clearAll() 后 size=0，实际=" + NpcLodStore.size());
        assertNull(NpcLodStore.get(1), "期望 clearAll 后 get(1) 返回 null");
        assertNull(NpcLodStore.get(2), "期望 clearAll 后 get(2) 返回 null");
    }

    // ── size ──────────────────────────────────────────────────────────────────

    @Test
    void sizeTracksUpsertAndRemove() {
        assertEquals(0, NpcLodStore.size(), "期望初始 size=0");

        NpcLodStore.upsert(new NpcLodSnapshot(1, "rogue", "引气", 1.0f, 0, 0, 0));
        assertEquals(1, NpcLodStore.size(), "期望 upsert 一条后 size=1，实际=" + NpcLodStore.size());

        NpcLodStore.upsert(new NpcLodSnapshot(2, "zombie", "醒灵", 1.0f, 0, 0, 0));
        assertEquals(2, NpcLodStore.size(), "期望 upsert 两条后 size=2，实际=" + NpcLodStore.size());

        NpcLodStore.remove(1);
        assertEquals(1, NpcLodStore.size(), "期望 remove(1) 后 size=1，实际=" + NpcLodStore.size());
    }

    // ── snapshot ──────────────────────────────────────────────────────────────

    @Test
    void snapshotReturnsSortedByEntityId() {
        NpcLodStore.upsert(new NpcLodSnapshot(30, "beast", "凝脉", 0.6f, 0, 0, 0));
        NpcLodStore.upsert(new NpcLodSnapshot(10, "rogue", "引气", 0.8f, 0, 0, 0));
        NpcLodStore.upsert(new NpcLodSnapshot(20, "zombie", "醒灵", 0.4f, 0, 0, 0));

        List<NpcLodSnapshot> snaps = List.copyOf(NpcLodStore.snapshot());
        assertEquals(3, snaps.size(), "期望 snapshot 返回 3 条，实际=" + snaps.size());
        assertEquals(10, snaps.get(0).entityId(), "期望第一条 entityId=10（按升序），实际=" + snaps.get(0).entityId());
        assertEquals(20, snaps.get(1).entityId(), "期望第二条 entityId=20，实际=" + snaps.get(1).entityId());
        assertEquals(30, snaps.get(2).entityId(), "期望第三条 entityId=30，实际=" + snaps.get(2).entityId());
    }

    @Test
    void snapshotOnEmptyStoreReturnsEmpty() {
        assertEquals(0, NpcLodStore.snapshot().size(),
            "期望空 store 的 snapshot() 返回空集合，实际 size=" + NpcLodStore.snapshot().size());
    }
}
