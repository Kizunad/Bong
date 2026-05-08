package com.bong.client.cultivation.voidaction;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

public final class VoidActionStore {
    private static final String DEFAULT_ZONE_ID = "spawn";
    private static final CopyOnWriteArrayList<Consumer<Snapshot>> LISTENERS = new CopyOnWriteArrayList<>();
    private static volatile Snapshot snapshot = Snapshot.empty();

    private VoidActionStore() {}

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.empty() : next;
        for (Consumer<Snapshot> listener : LISTENERS) {
            listener.accept(snapshot);
        }
    }

    public static void setTargetZone(String zoneId) {
        Snapshot current = snapshot;
        replace(current.withTargetZone(zoneId));
    }

    public static void setLegacyDraft(String inheritorId, List<Long> itemInstanceIds, String message) {
        Snapshot current = snapshot;
        replace(new Snapshot(
            current.targetZoneId(),
            current.cooldownReadyAtTicks(),
            sanitize(inheritorId, "heir"),
            itemInstanceIds == null ? List.of() : List.copyOf(itemInstanceIds),
            message == null || message.isBlank() ? null : message.trim()
        ));
    }

    public static void markDispatched(VoidActionKind kind, long nowTick) {
        Snapshot current = snapshot;
        EnumMap<VoidActionKind, Long> next = new EnumMap<>(VoidActionKind.class);
        next.putAll(current.cooldownReadyAtTicks());
        if (kind.cooldownTicks() > 0L) {
            next.put(kind, nowTick + kind.cooldownTicks());
        }
        replace(new Snapshot(
            current.targetZoneId(),
            next,
            current.legacyInheritorId(),
            current.legacyItemInstanceIds(),
            current.legacyMessage()
        ));
    }

    public static void addListener(Consumer<Snapshot> listener) {
        LISTENERS.add(listener);
    }

    public static void removeListener(Consumer<Snapshot> listener) {
        LISTENERS.remove(listener);
    }

    public static void resetForTests() {
        LISTENERS.clear();
        snapshot = Snapshot.empty();
    }

    private static String sanitize(String value, String fallback) {
        return value == null || value.isBlank() ? fallback : value.trim();
    }

    public record Snapshot(
        String targetZoneId,
        Map<VoidActionKind, Long> cooldownReadyAtTicks,
        String legacyInheritorId,
        List<Long> legacyItemInstanceIds,
        String legacyMessage
    ) {
        public Snapshot {
            targetZoneId = sanitize(targetZoneId, DEFAULT_ZONE_ID);
            EnumMap<VoidActionKind, Long> cooldowns = new EnumMap<>(VoidActionKind.class);
            if (cooldownReadyAtTicks != null) {
                cooldowns.putAll(cooldownReadyAtTicks);
            }
            cooldownReadyAtTicks = Map.copyOf(cooldowns);
            legacyInheritorId = sanitize(legacyInheritorId, "heir");
            legacyItemInstanceIds = legacyItemInstanceIds == null ? List.of() : List.copyOf(legacyItemInstanceIds);
            legacyMessage = legacyMessage == null || legacyMessage.isBlank() ? null : legacyMessage.trim();
        }

        public static Snapshot empty() {
            return new Snapshot(DEFAULT_ZONE_ID, Map.of(), "heir", List.of(), null);
        }

        public Snapshot withTargetZone(String zoneId) {
            return new Snapshot(zoneId, cooldownReadyAtTicks, legacyInheritorId, legacyItemInstanceIds, legacyMessage);
        }

        public long readyAtTick(VoidActionKind kind) {
            return cooldownReadyAtTicks.getOrDefault(kind, 0L);
        }

        public boolean ready(VoidActionKind kind, long nowTick) {
            return nowTick >= readyAtTick(kind);
        }
    }
}
