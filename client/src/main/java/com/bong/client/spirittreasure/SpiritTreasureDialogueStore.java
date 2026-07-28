package com.bong.client.spirittreasure;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public final class SpiritTreasureDialogueStore {
    private static final int MAX_DIALOGUES_PER_TREASURE = 24;
    private static final Map<String, Deque<SpiritTreasureDialogue>> DIALOGUES = new HashMap<>();

    private SpiritTreasureDialogueStore() {
    }

    public static synchronized void append(SpiritTreasureDialogue dialogue) {
        if (dialogue == null || dialogue.treasureId().isBlank()) {
            return;
        }
        Deque<SpiritTreasureDialogue> entries = DIALOGUES.computeIfAbsent(dialogue.treasureId(), ignored -> new ArrayDeque<>());
        entries.addLast(dialogue);
        while (entries.size() > MAX_DIALOGUES_PER_TREASURE) {
            entries.removeFirst();
        }
    }

    public static synchronized List<SpiritTreasureDialogue> recentFor(String treasureId) {
        Deque<SpiritTreasureDialogue> entries = DIALOGUES.get(treasureId);
        if (entries == null) {
            return List.of();
        }
        return new ArrayList<>(entries);
    }

    public static synchronized SpiritTreasureDialogue latestFor(String treasureId) {
        Deque<SpiritTreasureDialogue> entries = DIALOGUES.get(treasureId);
        return entries == null ? null : entries.peekLast();
    }

    public static synchronized void clear() {
        DIALOGUES.clear();
    }

    public static void resetForTests() {
        clear();
    }
}
