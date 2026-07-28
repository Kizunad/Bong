package com.bong.client.inventory.state;

import com.bong.client.inventory.model.MeridianBody;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * Global volatile snapshot store for meridian body state.
 * Server network handlers call {@link #replace(MeridianBody)} to push new data;
 * UI components subscribe via {@link #addListener(Consumer)} to react.
 */
public final class MeridianStateStore {
    private static volatile MeridianBody snapshot;
    private static final List<Consumer<MeridianBody>> listeners = new CopyOnWriteArrayList<>();

    private MeridianStateStore() {}

    public static MeridianBody snapshot() {
        return snapshot;
    }

    public static void replace(MeridianBody next) {
        snapshot = next;
        for (Consumer<MeridianBody> listener : listeners) {
            listener.accept(next);
        }
    }

    public static void addListener(Consumer<MeridianBody> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<MeridianBody> listener) {
        listeners.remove(listener);
    }

    /**
     * 断线时仅清当前会话快照，保留长期 UI listener wiring。
     */
    public static void clearOnDisconnect() {
        replace(null);
    }

    public static void resetForTests() {
        snapshot = null;
        listeners.clear();
    }
}
