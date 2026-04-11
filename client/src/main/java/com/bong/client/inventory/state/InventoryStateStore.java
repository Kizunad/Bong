package com.bong.client.inventory.state;

import com.bong.client.inventory.model.InventoryModel;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * Global volatile snapshot store for inventory state.
 * Server network handlers call {@link #replace(InventoryModel)} to push new data;
 * UI components subscribe via {@link #addListener(Consumer)} to react.
 */
public final class InventoryStateStore {
    private static volatile InventoryModel snapshot = InventoryModel.empty();
    private static final List<Consumer<InventoryModel>> listeners = new CopyOnWriteArrayList<>();

    private InventoryStateStore() {}

    public static InventoryModel snapshot() {
        return snapshot;
    }

    public static void replace(InventoryModel next) {
        InventoryModel value = next == null ? InventoryModel.empty() : next;
        snapshot = value;
        for (Consumer<InventoryModel> listener : listeners) {
            listener.accept(value);
        }
    }

    /** Subscribe to snapshot changes. Listener is called on the thread that calls replace(). */
    public static void addListener(Consumer<InventoryModel> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<InventoryModel> listener) {
        listeners.remove(listener);
    }

    public static void resetForTests() {
        snapshot = InventoryModel.empty();
        listeners.clear();
    }
}
