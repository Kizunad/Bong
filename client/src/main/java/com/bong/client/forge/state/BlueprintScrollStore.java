package com.bong.client.forge.state;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/** plan-forge-v1 §1.4 — 已学图谱书本地 Store。 */
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

    public static void turn(int delta) {
        if (learned.isEmpty()) return;
        currentIndex = ((currentIndex + delta) % learned.size() + learned.size()) % learned.size();
    }

    public static Entry current() {
        if (learned.isEmpty()) return null;
        if (currentIndex < 0 || currentIndex >= learned.size()) return null;
        return learned.get(currentIndex);
    }

    public static void resetForTests() {
        learned = new CopyOnWriteArrayList<>();
        currentIndex = 0;
    }
}
