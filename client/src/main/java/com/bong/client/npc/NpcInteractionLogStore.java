package com.bong.client.npc;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

public final class NpcInteractionLogStore {
    public static final int MAX_ENTRIES = 10;
    private static final List<NpcInteractionLogEntry> ENTRIES = new ArrayList<>();
    private static boolean visible;

    private NpcInteractionLogStore() {
    }

    public static synchronized void record(NpcInteractionLogEntry entry) {
        if (entry == null) {
            return;
        }
        ENTRIES.removeIf(existing -> existing.entityId() == entry.entityId());
        ENTRIES.add(entry);
        ENTRIES.sort(Comparator.comparingLong(NpcInteractionLogEntry::observedAtMillis).reversed());
        if (ENTRIES.size() > MAX_ENTRIES) {
            ENTRIES.subList(MAX_ENTRIES, ENTRIES.size()).clear();
        }
    }

    public static synchronized List<NpcInteractionLogEntry> snapshot() {
        return List.copyOf(ENTRIES);
    }

    public static synchronized void toggleVisible() {
        visible = !visible;
    }

    public static synchronized boolean visible() {
        return visible;
    }

    public static synchronized void resetForTests() {
        ENTRIES.clear();
        visible = false;
    }
}
