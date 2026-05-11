package com.bong.client.gathering;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

public final class GatheringSessionStore {
    private static volatile GatheringSessionViewModel snapshot = GatheringSessionViewModel.empty();
    private static final List<Consumer<GatheringSessionViewModel>> listeners = new CopyOnWriteArrayList<>();

    private GatheringSessionStore() {
    }

    public static GatheringSessionViewModel snapshot() {
        return snapshot;
    }

    public static void replace(GatheringSessionViewModel next) {
        snapshot = next == null ? GatheringSessionViewModel.empty() : next;
        for (Consumer<GatheringSessionViewModel> listener : listeners) {
            listener.accept(snapshot);
        }
    }

    public static void clearOnDisconnect() {
        replace(GatheringSessionViewModel.empty());
    }

    public static void addListener(Consumer<GatheringSessionViewModel> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<GatheringSessionViewModel> listener) {
        listeners.remove(listener);
    }

    public static void resetForTests() {
        snapshot = GatheringSessionViewModel.empty();
        listeners.clear();
    }
}
