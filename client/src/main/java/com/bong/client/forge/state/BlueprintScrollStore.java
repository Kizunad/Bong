package com.bong.client.forge.state;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * plan-forge-v1 §1.4 — 已学图谱书本地 Store。
 *
 * <p>plan-forge-session-entry-wiring-v1 §4.1#2 —— server 权威页码：本 Store 只做 S2C
 * {@code forge_blueprint_book} 快照的只读镜像（经 {@code ForgeBlueprintBookHandler} 写入），
 * <b>不提供本地翻页</b>。翻页由 client 发 {@code forge_blueprint_turn_page} C2S，页码变化
 * 只能通过 server 回推的下一条快照体现——不做本地乐观 + 校正双路径。</p>
 */
public final class BlueprintScrollStore {
    public record Entry(String id, String displayName, int tierCap, int stepCount) {}

    private static volatile List<Entry> learned = new CopyOnWriteArrayList<>();
    private static volatile int currentIndex = 0;

    private BlueprintScrollStore() {}

    public static List<Entry> entries() {
        return List.copyOf(learned);
    }

    public static int currentIndex() {
        return currentIndex;
    }

    public static void replace(List<Entry> next, int nextIndex) {
        learned = new CopyOnWriteArrayList<>(next != null ? next : new ArrayList<>());
        currentIndex = Math.max(0, Math.min(nextIndex, learned.size() - 1));
    }

    public static Entry current() {
        if (learned.isEmpty()) return null;
        if (currentIndex < 0 || currentIndex >= learned.size()) return null;
        return learned.get(currentIndex);
    }

    public static void clearOnDisconnect() {
        learned = new CopyOnWriteArrayList<>();
        currentIndex = 0;
    }

    public static void resetForTests() {
        learned = new CopyOnWriteArrayList<>();
        currentIndex = 0;
    }
}
