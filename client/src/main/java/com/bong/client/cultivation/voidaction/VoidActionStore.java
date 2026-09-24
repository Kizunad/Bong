package com.bong.client.cultivation.voidaction;

import java.util.EnumMap;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;
import java.util.function.UnaryOperator;

public final class VoidActionStore {
    private static final String DEFAULT_ZONE_ID = "spawn";
    private static final CopyOnWriteArrayList<Consumer<Snapshot>> LISTENERS = new CopyOnWriteArrayList<>();
    private static final AtomicReference<Snapshot> SNAPSHOT = new AtomicReference<>(Snapshot.empty());

    private VoidActionStore() {}

    public static Snapshot snapshot() {
        return SNAPSHOT.get();
    }

    public static void replace(Snapshot next) {
        Snapshot normalized = next == null ? Snapshot.empty() : next;
        SNAPSHOT.set(normalized);
        notifyListeners(normalized);
    }

    private static void update(UnaryOperator<Snapshot> updater) {
        Snapshot next = SNAPSHOT.updateAndGet(current -> updater.apply(current == null ? Snapshot.empty() : current));
        notifyListeners(next);
    }

    private static void notifyListeners(Snapshot snapshot) {
        for (Consumer<Snapshot> listener : LISTENERS) {
            listener.accept(snapshot);
        }
    }

    public static void setTargetZone(String zoneId) {
        update(current -> current.withTargetZone(zoneId));
    }

    public static void markDispatched(VoidActionKind kind, long nowTick) {
        update(current -> {
            EnumMap<VoidActionKind, Long> next = new EnumMap<>(VoidActionKind.class);
            next.putAll(current.cooldownReadyAtTicks());
            if (kind.cooldownTicks() > 0L) {
                next.put(kind, nowTick + kind.cooldownTicks());
            }
            return new Snapshot(current.targetZoneId(), next);
        });
    }

    public static void addListener(Consumer<Snapshot> listener) {
        LISTENERS.add(listener);
    }

    public static void removeListener(Consumer<Snapshot> listener) {
        LISTENERS.remove(listener);
    }

    /** 断线时仅替换会话快照，保留并通知长期 listener。 */
    public static void clearOnDisconnect() {
        replace(Snapshot.empty());
    }

    public static void resetForTests() {
        LISTENERS.clear();
        SNAPSHOT.set(Snapshot.empty());
    }

    private static String sanitize(String value, String fallback) {
        return value == null || value.isBlank() ? fallback : value.trim();
    }

    public record Snapshot(
        String targetZoneId,
        Map<VoidActionKind, Long> cooldownReadyAtTicks
    ) {
        public Snapshot {
            targetZoneId = sanitize(targetZoneId, DEFAULT_ZONE_ID);
            EnumMap<VoidActionKind, Long> cooldowns = new EnumMap<>(VoidActionKind.class);
            if (cooldownReadyAtTicks != null) {
                cooldowns.putAll(cooldownReadyAtTicks);
            }
            cooldownReadyAtTicks = Map.copyOf(cooldowns);
        }

        public static Snapshot empty() {
            return new Snapshot(DEFAULT_ZONE_ID, Map.of());
        }

        public Snapshot withTargetZone(String zoneId) {
            return new Snapshot(zoneId, cooldownReadyAtTicks);
        }

        public long readyAtTick(VoidActionKind kind) {
            return cooldownReadyAtTicks.getOrDefault(kind, 0L);
        }

        public boolean ready(VoidActionKind kind, long nowTick) {
            return nowTick >= readyAtTick(kind);
        }
    }
}
