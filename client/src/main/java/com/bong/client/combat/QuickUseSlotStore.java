package com.bong.client.combat;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/** Client-side store for F1-F9 quick-use slot config (§11.1). */
public final class QuickUseSlotStore {
    public enum Source {
        LOCAL,
        SERVER
    }

    public record Update(
        QuickSlotConfig config,
        Source source,
        String ackRequestId,
        Boolean bindAccepted,
        long sequence
    ) {}

    private static final Object LOCK = new Object();
    private static volatile QuickSlotConfig snapshot = QuickSlotConfig.empty();
    private static volatile Update latest =
        new Update(snapshot, Source.SERVER, null, null, 0L);
    private static long sequence;
    private static final List<Consumer<Update>> listeners = new CopyOnWriteArrayList<>();

    private QuickUseSlotStore() {
    }

    public static QuickSlotConfig snapshot() {
        return snapshot;
    }

    public static void replace(QuickSlotConfig next) {
        replaceAuthoritative(next, null, null);
    }

    public static void replaceAuthoritative(
        QuickSlotConfig next,
        String ackRequestId,
        Boolean bindAccepted
    ) {
        publish(next, Source.SERVER, ackRequestId, bindAccepted);
    }

    public static void replaceLocal(QuickSlotConfig next) {
        publish(next, Source.LOCAL, null, null);
    }

    /** 原子完成“先订阅、再读取快照”，并用 sequence 让调用方丢弃竞态旧值。 */
    public static Update subscribeAndGet(Consumer<Update> listener) {
        synchronized (LOCK) {
            listeners.add(listener);
            return latest;
        }
    }

    public static void addListener(Consumer<Update> listener) {
        subscribeAndGet(listener);
    }

    public static void removeListener(Consumer<Update> listener) {
        listeners.remove(listener);
    }

    /**
     * Clears the session snapshot while retaining listeners and the monotonic
     * sequence used by subscribers to reject stale updates.
     */
    public static void clearOnDisconnect() {
        publish(QuickSlotConfig.empty(), Source.LOCAL, null, null);
    }

    public static void resetForTests() {
        synchronized (LOCK) {
            snapshot = QuickSlotConfig.empty();
            sequence = 0L;
            latest = new Update(snapshot, Source.SERVER, null, null, sequence);
            listeners.clear();
        }
    }

    private static void publish(
        QuickSlotConfig next,
        Source source,
        String ackRequestId,
        Boolean bindAccepted
    ) {
        Update update;
        synchronized (LOCK) {
            snapshot = next == null ? QuickSlotConfig.empty() : next;
            sequence++;
            update = new Update(snapshot, source, ackRequestId, bindAccepted, sequence);
            latest = update;
        }
        for (Consumer<Update> listener : listeners) {
            listener.accept(update);
        }
    }
}
