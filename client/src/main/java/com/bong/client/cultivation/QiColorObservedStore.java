package com.bong.client.cultivation;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

public final class QiColorObservedStore {
    private static volatile QiColorObservedState snapshot;
    private static final List<Consumer<QiColorObservedState>> listeners = new CopyOnWriteArrayList<>();

    private QiColorObservedStore() {}

    public static QiColorObservedState snapshot() {
        return snapshot;
    }

    public static void replace(QiColorObservedState next) {
        snapshot = next;
        for (Consumer<QiColorObservedState> listener : listeners) {
            listener.accept(next);
        }
    }

    public static void clear() {
        replace(null);
    }

    public static void addListener(Consumer<QiColorObservedState> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<QiColorObservedState> listener) {
        listeners.remove(listener);
    }

    public static void resetForTests() {
        snapshot = null;
        listeners.clear();
    }
}
