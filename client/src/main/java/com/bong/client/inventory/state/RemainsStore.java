package com.bong.client.inventory.state;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Client-side observable set of world remains containers derived from {@code remains_sync}
 * (plan-remains-suite P0)。照 {@link DroppedItemStore} 的形状——同样的 insertionOrder
 * tie-break 理由：marker 渲染目标与 G 键 pickup 目标必须一致，避免距离相等时 HashMap
 * 迭代顺序造成目标抖动。
 *
 * <p>与 {@link DroppedItemStore} 的关键差异：这里的 key 是 {@code remainsId}（服务端遗骸
 * 实体的 UUID 字符串），不是 {@code long instanceId}——遗骸不是背包物品，没有
 * instance_id。</p>
 */
public final class RemainsStore {

    /** 距离平方差在此阈值内视为等距，触发 insertionOrder tie-breaker（约 0.1 m 量级）。 */
    static final double DISTANCE_TIE_EPSILON_SQ = 0.01;

    public record Entry(
        String remainsId,
        double worldPosX,
        double worldPosY,
        double worldPosZ,
        String dimension,
        String displayName,
        int itemCount,
        long boneCoins
    ) {}

    private static final Map<String, Entry> entries = new ConcurrentHashMap<>();
    private static final Map<String, Long> insertionOrders = new ConcurrentHashMap<>();
    private static final AtomicLong insertionCounter = new AtomicLong(0L);

    private RemainsStore() {}

    public static List<Entry> snapshot() {
        return List.copyOf(new ArrayList<>(entries.values()));
    }

    public static Entry get(String remainsId) {
        return remainsId == null ? null : entries.get(remainsId);
    }

    /**
     * 最近遗骸。距离平方差在 {@link #DISTANCE_TIE_EPSILON_SQ} 内视为等距，
     * 按 insertionOrder 倒序（新的优先）作 tie-breaker。
     */
    public static Entry nearestTo(double x, double y, double z) {
        Entry nearest = null;
        double bestDistanceSq = Double.POSITIVE_INFINITY;
        long bestOrder = Long.MIN_VALUE;
        for (Entry entry : entries.values()) {
            if (entry == null || entry.remainsId() == null) {
                continue;
            }
            double distanceSq = distanceSq(x, y, z, entry);
            long order = insertionOrders.getOrDefault(entry.remainsId(), 0L);
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
        if (entry == null || entry.remainsId() == null) {
            return;
        }
        // 先注册 order 再 put，避免 reader 看到 entry 却读不到 order。
        insertionOrders.computeIfAbsent(entry.remainsId(), k -> insertionCounter.incrementAndGet());
        entries.put(entry.remainsId(), entry);
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

    public static void remove(String remainsId) {
        if (remainsId == null) {
            return;
        }
        entries.remove(remainsId);
        insertionOrders.remove(remainsId);
    }

    public static void clearOnDisconnect() {
        entries.clear();
        insertionOrders.clear();
    }

    static long insertionCounterForTests() {
        return insertionCounter.get();
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
