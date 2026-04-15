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
    private static volatile boolean authoritativeLoaded = false;
    private static volatile long revision = -1L;
    private static final List<Consumer<InventoryModel>> listeners = new CopyOnWriteArrayList<>();

    private InventoryStateStore() {}

    public static InventoryModel snapshot() {
        return snapshot;
    }

    public static boolean isAuthoritativeLoaded() {
        return authoritativeLoaded;
    }

    public static long revision() {
        return revision;
    }

    public static void replace(InventoryModel next) {
        InventoryModel value = next == null ? InventoryModel.empty() : next;
        snapshot = value;
        authoritativeLoaded = false;
        revision = 0L;
        notifyListeners(value);
    }

    public static void clearOnDisconnect() {
        snapshot = InventoryModel.empty();
        authoritativeLoaded = false;
        revision = -1L;
        notifyListeners(snapshot);
    }

    public static void applyAuthoritativeSnapshot(InventoryModel next, long nextRevision) {
        snapshot = next == null ? InventoryModel.empty() : next;
        authoritativeLoaded = true;
        revision = Math.max(0L, nextRevision);
        notifyListeners(snapshot);
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
        authoritativeLoaded = false;
        revision = -1L;
        listeners.clear();
    }

    private static void notifyListeners(InventoryModel value) {
        for (Consumer<InventoryModel> listener : listeners) {
            listener.accept(value);
        }
    }
}
