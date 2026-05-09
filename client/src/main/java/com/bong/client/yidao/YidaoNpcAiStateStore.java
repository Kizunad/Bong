package com.bong.client.yidao;

import java.util.LinkedHashMap;
import java.util.Map;

/** Client-side cache for {@code healer_npc_ai_state}. */
public final class YidaoNpcAiStateStore {
    private static final String CLEAR_ACTION = "clear";

    public record Snapshot(
        String healerId,
        String activeAction,
        int queueLen,
        int reputation,
        boolean retreating
    ) {
        public static final Snapshot EMPTY = new Snapshot("", "", 0, 0, false);

        public Snapshot {
            healerId = healerId == null ? "" : healerId;
            activeAction = activeAction == null ? "" : activeAction;
            queueLen = Math.max(0, queueLen);
        }

        public boolean active() {
            return !healerId.isBlank() && !clearSignal();
        }

        public boolean clearSignal() {
            return CLEAR_ACTION.equals(activeAction);
        }
    }

    private static volatile Map<String, Snapshot> snapshots = Map.of();

    private YidaoNpcAiStateStore() {
    }

    public static Snapshot snapshot() {
        return snapshots.values().stream().findFirst().orElse(Snapshot.EMPTY);
    }

    public static Snapshot snapshot(String healerId) {
        if (healerId == null || healerId.isBlank()) {
            return Snapshot.EMPTY;
        }
        return snapshots.getOrDefault(healerId, Snapshot.EMPTY);
    }

    public static int activeCount() {
        return snapshots.size();
    }

    public static void upsert(Snapshot next) {
        if (next == null || next.healerId().isBlank()) {
            return;
        }
        Map<String, Snapshot> copy = new LinkedHashMap<>(snapshots);
        if (next.clearSignal()) {
            copy.remove(next.healerId());
        } else {
            copy.put(next.healerId(), next);
        }
        snapshots = copy;
    }

    public static void replace(Snapshot next) {
        snapshots = Map.of();
        upsert(next);
    }

    public static void resetForTests() {
        snapshots = Map.of();
    }
}
