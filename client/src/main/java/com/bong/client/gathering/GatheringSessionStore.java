package com.bong.client.gathering;

import com.bong.client.BongClient;

import java.util.List;
import java.util.Objects;
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
        GatheringSessionViewModel current = next == null ? GatheringSessionViewModel.empty() : next;
        snapshot = current;
        for (Consumer<GatheringSessionViewModel> listener : listeners) {
            try {
                listener.accept(current);
            } catch (RuntimeException error) {
                BongClient.LOGGER.warn("Gathering session listener failed for {}", current.sessionId(), error);
            }
        }
    }

    public static void clear(String sessionId) {
        String normalizedSessionId = sessionId == null ? "" : sessionId.trim();
        GatheringSessionViewModel current = snapshot;
        if (!normalizedSessionId.isEmpty() && normalizedSessionId.equals(current.sessionId())) {
            replace(GatheringSessionViewModel.empty());
        }
    }

    public static void clearOnDisconnect() {
        replace(GatheringSessionViewModel.empty());
    }

    public static void addListener(Consumer<GatheringSessionViewModel> listener) {
        listeners.add(Objects.requireNonNull(listener, "listener"));
    }

    public static void removeListener(Consumer<GatheringSessionViewModel> listener) {
        listeners.remove(listener);
    }

    public static void resetForTests() {
        snapshot = GatheringSessionViewModel.empty();
        listeners.clear();
    }
}
