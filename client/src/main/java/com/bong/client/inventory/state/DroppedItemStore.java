package com.bong.client.inventory.state;

import com.bong.client.inventory.model.InventoryItem;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Client-side observable set of ground-dropped items derived from inventory_event:dropped.
 *
 * <p>这是 P4 的过渡数据源：先把“从背包移除”与“地上存在掉落物”分离出来，后续无论接
 * debug 面板还是 world-entity 渲染，都从这里读。</p>
 *
 * <p>HudPlanner 渲染 marker 与 G 键 pickup 两条路径都走 {@link #nearestTo}，
 * 为避免距离相等时 HashMap 迭代顺序造成目标抖动（marker 渲染目标与捡起目标可能不一致），
 * 这里维护一个单调递增的 insertion order，作为距离相等时的 tie-breaker——**最新到达的掉落物优先**，
 * 与玩家"刚丢的物品最想被 marker 点亮 / 被 G 捡回"的直觉一致。</p>
 */
public final class DroppedItemStore {

    /** 距离平方差在此阈值内视为等距，触发 insertionOrder tie-breaker（约 0.1 m 量级）。 */
    static final double DISTANCE_TIE_EPSILON_SQ = 0.01;

    public record Entry(
        long instanceId,
        String sourceContainerId,
        int sourceRow,
        int sourceCol,
        double worldPosX,
        double worldPosY,
        double worldPosZ,
        InventoryItem item
    ) {}

    private static final Map<Long, Entry> entries = new ConcurrentHashMap<>();
    private static final Map<Long, Long> insertionOrders = new ConcurrentHashMap<>();
    private static final AtomicLong insertionCounter = new AtomicLong(0L);

    private DroppedItemStore() {}

    public static List<Entry> snapshot() {
        return List.copyOf(new ArrayList<>(entries.values()));
    }

    public static Entry get(long instanceId) {
        return entries.get(instanceId);
    }

    /**
     * 最近掉落物。距离平方差在 {@link #DISTANCE_TIE_EPSILON_SQ} 内视为等距，
     * 按 insertionOrder 倒序（新的优先）作 tie-breaker。
     */
    public static Entry nearestTo(double x, double y, double z) {
        Entry nearest = null;
        double bestDistanceSq = Double.POSITIVE_INFINITY;
        long bestOrder = Long.MIN_VALUE;
        for (Entry entry : entries.values()) {
            if (entry == null || entry.item() == null) {
                continue;
            }
            double distanceSq = distanceSq(x, y, z, entry);
            long order = insertionOrders.getOrDefault(entry.instanceId(), 0L);
            if (isStrictlyCloser(distanceSq, bestDistanceSq)
                || (isTie(distanceSq, bestDistanceSq) && order > bestOrder)) {
                bestDistanceSq = distanceSq;
                bestOrder = order;
                nearest = entry;
            }
        }
        return nearest;
    }

    public static void putOrReplace(Entry entry) {
        if (entry == null || entry.item() == null) {
            return;
        }
        // 先注册 order 再 put，避免 reader 看到 entry 却读不到 order。
        insertionOrders.computeIfAbsent(entry.instanceId(), k -> insertionCounter.incrementAndGet());
        entries.put(entry.instanceId(), entry);
    }

    public static void replaceAll(List<Entry> newEntries) {
        entries.clear();
        insertionOrders.clear();
        if (newEntries == null) {
            return;
        }
        // server 发来的 list 顺序即权威时间序，按序分配 insertionOrder。
        for (Entry entry : newEntries) {
            putOrReplace(entry);
        }
    }

    public static void remove(long instanceId) {
        entries.remove(instanceId);
        insertionOrders.remove(instanceId);
    }

    public static void clearOnDisconnect() {
        entries.clear();
        insertionOrders.clear();
    }

    public static void resetForTests() {
        entries.clear();
        insertionOrders.clear();
        insertionCounter.set(0L);
    }

    private static double distanceSq(double x, double y, double z, Entry entry) {
        double dx = x - entry.worldPosX();
        double dy = y - entry.worldPosY();
        double dz = z - entry.worldPosZ();
        return dx * dx + dy * dy + dz * dz;
    }

    private static boolean isStrictlyCloser(double candidateSq, double bestSq) {
        return candidateSq + DISTANCE_TIE_EPSILON_SQ < bestSq;
    }

    private static boolean isTie(double candidateSq, double bestSq) {
        return Math.abs(candidateSq - bestSq) <= DISTANCE_TIE_EPSILON_SQ;
    }
}
