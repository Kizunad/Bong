package com.bong.client.inventory.state;

import com.bong.client.inventory.model.PhysicalBody;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * Global volatile snapshot store for physical body state.
 * Server network handlers call {@link #replace(PhysicalBody)} to push new data.
 */
public final class PhysicalBodyStore {
    private static volatile PhysicalBody snapshot;
    private static final List<Consumer<PhysicalBody>> listeners = new CopyOnWriteArrayList<>();

    private PhysicalBodyStore() {}

    public static PhysicalBody snapshot() { return snapshot; }

    public static void replace(PhysicalBody next) {
        snapshot = next;
        for (Consumer<PhysicalBody> listener : listeners) {
            listener.accept(next);
        }
    }

    public static void addListener(Consumer<PhysicalBody> listener) { listeners.add(listener); }
    public static void removeListener(Consumer<PhysicalBody> listener) { listeners.remove(listener); }

    public static void resetForTests() {
        snapshot = null;
        listeners.clear();
    }
}
