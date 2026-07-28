package com.bong.client.alchemy.state;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

// plan-alchemy-v1 P6 — 炼丹会话快照本地 Store。
public final class AlchemySessionStore {
    public record StageHint(int atTick, int window, String summary, boolean completed, boolean missed) {}

    public record Snapshot(
        String recipeId,
        boolean active,
        int elapsedTicks,
        int targetTicks,
        float tempCurrent,
        float tempTarget,
        float tempBand,
        double qiInjected,
        double qiTarget,
        String statusLabel,
        List<StageHint> stages,
        List<String> interventionLog
    ) {
        public static Snapshot empty() {
            return new Snapshot(
                "", false, 0, 0, 0.0f, 0.0f, 0.0f, 0.0, 0.0,
                "",
                List.of(),
                List.of()
            );
        }

        public boolean isActive() {
            return active
                && recipeId != null
                && !recipeId.isBlank()
                && targetTicks > 0;
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();
    private static final List<Consumer<Snapshot>> listeners = new CopyOnWriteArrayList<>();

    private AlchemySessionStore() {
    }

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.empty() : next;
        notifyListeners(snapshot);
    }

    /** Subscribe to session changes. Listener runs on the thread that calls {@link #replace}. */
    public static void addListener(Consumer<Snapshot> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<Snapshot> listener) {
        listeners.remove(listener);
    }

    public static void clearOnDisconnect() {
        replace(null);
    }

    public static void resetForTests() {
        snapshot = Snapshot.empty();
        listeners.clear();
    }

    public static int listenerCountForTests() {
        return listeners.size();
    }

    private static void notifyListeners(Snapshot value) {
        for (Consumer<Snapshot> listener : listeners) {
            listener.accept(value);
        }
    }
}
