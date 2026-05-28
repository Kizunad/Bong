package com.bong.client.hud;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

public final class LootContainerStateStore {

    public sealed interface Session permits OpenSession, Closed {}

    public record OpenSession(
        long sessionId,
        String sourceKind,
        String grade,
        int rows,
        int cols,
        long timeoutWallSecs,
        List<InventoryModel.GridEntry> placedItems
    ) implements Session {}

    public record Closed(long sessionId, String reason) implements Session {}

    @FunctionalInterface
    public interface Listener {
        void onLootContainerChanged(Session session);
    }

    private static volatile Session current = null;
    private static final List<Listener> listeners = new CopyOnWriteArrayList<>();

    private LootContainerStateStore() {}

    public static Session current() {
        return current;
    }

    public static boolean isOpen() {
        return current instanceof OpenSession;
    }

    public static void open(OpenSession session) {
        current = session;
        notifyListeners(session);
    }

    public static void update(long sessionId, List<InventoryModel.GridEntry> placedItems) {
        Session prev = current;
        if (prev instanceof OpenSession open && open.sessionId() == sessionId) {
            OpenSession updated = new OpenSession(
                open.sessionId(), open.sourceKind(), open.grade(),
                open.rows(), open.cols(), open.timeoutWallSecs(),
                placedItems
            );
            current = updated;
            notifyListeners(updated);
        }
    }

    public static void close(long sessionId, String reason) {
        Session prev = current;
        if (prev instanceof OpenSession open && open.sessionId() == sessionId) {
            Closed closed = new Closed(sessionId, reason);
            current = null;
            notifyListeners(closed);
        }
    }

    public static void clear() {
        current = null;
    }

    public static void addListener(Listener listener) {
        listeners.add(listener);
    }

    public static void removeListener(Listener listener) {
        listeners.remove(listener);
    }

    public static void resetForTests() {
        current = null;
        listeners.clear();
    }

    private static void notifyListeners(Session session) {
        for (Listener l : listeners) {
            l.onLootContainerChanged(session);
        }
    }
}
