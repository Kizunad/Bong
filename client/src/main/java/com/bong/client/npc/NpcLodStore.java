package com.bong.client.npc;

import java.util.Collection;
import java.util.Comparator;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * 远视野低 LOD NPC 快照存储（plan-offscreen-war-v1 §P7 PR-11）。
 *
 * <p>键：MC entity id（与 Valence entity id 对齐）。
 * <p>线程安全：{@code ConcurrentHashMap}，网络线程 write，渲染线程 read。
 *
 * <p>生命周期：
 * <ul>
 *   <li>服务端每 80 tick（约 4 秒）推一次 {@code bong:npc_lod} → {@link #upsert}
 *   <li>断线时 {@link #clearAll}（在 {@code BongNetworkHandler} DISCONNECT 回调中调用）
 * </ul>
 */
public final class NpcLodStore {
    private static final Map<Integer, NpcLodSnapshot> STORE = new ConcurrentHashMap<>();

    private NpcLodStore() {
    }

    /** 插入或更新 snapshot。{@code null} 入参静默忽略。 */
    public static void upsert(NpcLodSnapshot snapshot) {
        if (snapshot == null) {
            return;
        }
        STORE.put(snapshot.entityId(), snapshot);
    }

    /** 按 entity id 查询。找不到返回 {@code null}。 */
    public static NpcLodSnapshot get(int entityId) {
        return STORE.get(entityId);
    }

    /** 按 entity id 删除（NPC despawn 时调用）。 */
    public static void remove(int entityId) {
        STORE.remove(entityId);
    }

    /** 清空全部（断线 / 切服时调用）。 */
    public static void clearAll() {
        STORE.clear();
    }

    /** 当前快照数量（供测试 / debug 用）。 */
    public static int size() {
        return STORE.size();
    }

    /**
     * 按 entity id 排序的快照列表（供测试 / debug / 渲染迭代用）。
     *
     * <p>返回值是当前状态的不可变快照，不持有 store 锁。
     */
    public static Collection<NpcLodSnapshot> snapshot() {
        return STORE.values().stream()
            .sorted(Comparator.comparingInt(NpcLodSnapshot::entityId))
            .toList();
    }
}
