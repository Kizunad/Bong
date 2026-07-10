package com.bong.client.combat;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/** Client-side store for F1-F9 quick-use slot config (§11.1). */
public final class QuickUseSlotStore {
    private static volatile QuickSlotConfig snapshot = QuickSlotConfig.empty();
    private static final List<Consumer<QuickSlotConfig>> listeners = new CopyOnWriteArrayList<>();

    private QuickUseSlotStore() {
    }

    public static QuickSlotConfig snapshot() {
        return snapshot;
    }

    public static void replace(QuickSlotConfig next) {
        snapshot = next == null ? QuickSlotConfig.empty() : next;
        for (Consumer<QuickSlotConfig> listener : listeners) {
            listener.accept(snapshot);
        }
    }

    public static void addListener(Consumer<QuickSlotConfig> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<QuickSlotConfig> listener) {
        listeners.remove(listener);
    }

    public static void resetForTests() {
        snapshot = QuickSlotConfig.empty();
        listeners.clear();
    }
}
