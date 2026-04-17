package com.bong.client.botany;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

public final class HarvestSessionStore {
    private static volatile HarvestSessionViewModel snapshot = HarvestSessionViewModel.empty();
    private static final List<Consumer<HarvestSessionViewModel>> listeners = new CopyOnWriteArrayList<>();

    private HarvestSessionStore() {
    }

    public static HarvestSessionViewModel snapshot() {
        return snapshot;
    }

    public static void replace(HarvestSessionViewModel next) {
        snapshot = next == null ? HarvestSessionViewModel.empty() : next;
        notifyListeners(snapshot);
    }

    public static boolean capturesReservedInput() {
        return snapshot().interactive();
    }

    public static void requestMode(BotanyHarvestMode mode, long nowMillis) {
        if (mode == null) {
            return;
        }
        HarvestSessionViewModel current = snapshot;
        if (!current.interactive()) {
            return;
        }
        replace(current.withRequestedMode(mode, nowMillis));
    }

    public static void interruptLocally(String reason, long nowMillis) {
        HarvestSessionViewModel current = snapshot;
        if (!current.interactive() || current.mode() == null) {
            return;
        }
        replace(current.locallyInterrupted(reason, nowMillis));
    }

    public static void clearOnDisconnect() {
        replace(HarvestSessionViewModel.empty());
    }

    public static void addListener(Consumer<HarvestSessionViewModel> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<HarvestSessionViewModel> listener) {
        listeners.remove(listener);
    }

    public static void resetForTests() {
        snapshot = HarvestSessionViewModel.empty();
        listeners.clear();
    }

    private static void notifyListeners(HarvestSessionViewModel value) {
        for (Consumer<HarvestSessionViewModel> listener : listeners) {
            listener.accept(value);
        }
    }
}
