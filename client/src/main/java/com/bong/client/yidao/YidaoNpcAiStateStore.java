package com.bong.client.yidao;

/** Client-side cache for {@code healer_npc_ai_state}. */
public final class YidaoNpcAiStateStore {
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
    }

    private static volatile Snapshot snapshot = Snapshot.EMPTY;

    private YidaoNpcAiStateStore() {
    }

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.EMPTY : next;
    }

    public static void resetForTests() {
        snapshot = Snapshot.EMPTY;
    }
}
