package com.bong.client.skill;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

public final class SkillMilestoneStore {
    private static volatile List<SkillMilestoneSnapshot> snapshot = List.of();
    private static volatile String summary = "";
    private static final CopyOnWriteArrayList<Consumer<List<SkillMilestoneSnapshot>>> listeners =
        new CopyOnWriteArrayList<>();

    private SkillMilestoneStore() {}

    public static List<SkillMilestoneSnapshot> snapshot() {
        return snapshot;
    }

    public static String summary() {
        return summary;
    }

    public static void replace(List<SkillMilestoneSnapshot> next, String nextSummary) {
        snapshot = next == null ? List.of() : List.copyOf(next);
        summary = nextSummary == null ? "" : nextSummary;
        for (Consumer<List<SkillMilestoneSnapshot>> listener : listeners) {
            listener.accept(snapshot);
        }
    }

    public static void addListener(Consumer<List<SkillMilestoneSnapshot>> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<List<SkillMilestoneSnapshot>> listener) {
        listeners.remove(listener);
    }

    public static void clearOnDisconnect() {
        replace(List.of(), "");
    }

    public static void resetForTests() {
        snapshot = List.of();
        summary = "";
        listeners.clear();
    }
}
