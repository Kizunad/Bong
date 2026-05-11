package com.bong.client.omen;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;

public final class OmenStateStore {
    public enum Kind {
        PSEUDO_VEIN,
        BEAST_TIDE,
        REALM_COLLAPSE,
        KARMA_BACKLASH
    }

    public record Entry(
        Kind kind,
        double[] origin,
        double strength,
        long expiresAtMillis
    ) {
        public Entry {
            Objects.requireNonNull(kind, "kind");
            Objects.requireNonNull(origin, "origin");
            if (origin.length != 3) {
                throw new IllegalArgumentException("origin must be length 3");
            }
            origin = origin.clone();
            strength = clamp01(strength);
        }

        @Override
        public double[] origin() {
            return origin.clone();
        }

        public boolean activeAt(long nowMillis) {
            return nowMillis <= expiresAtMillis;
        }
    }

    public record Snapshot(List<Entry> entries) {
        public Snapshot {
            entries = List.copyOf(entries == null ? List.of() : entries);
        }

        public static Snapshot empty() {
            return new Snapshot(List.of());
        }
    }

    private static final List<Entry> entries = new ArrayList<>();
    private static final long MIN_VISIBLE_MS = 1_000L;
    private static final long MAX_VISIBLE_MS = 15_000L;

    private OmenStateStore() {
    }

    public static synchronized void note(VfxEventPayload.SpawnParticle payload, long nowMillis) {
        Kind kind = kindFromEventId(payload.eventId());
        if (kind == null) {
            return;
        }
        long durationMillis = payload.durationTicks().orElse(20) * 50L;
        durationMillis = Math.max(MIN_VISIBLE_MS, Math.min(MAX_VISIBLE_MS, durationMillis));
        double strength = payload.strength().orElse(0.6);
        upsert(new Entry(kind, payload.origin().clone(), strength, nowMillis + durationMillis));
    }

    public static synchronized Snapshot snapshot(long nowMillis) {
        entries.removeIf(entry -> !entry.activeAt(nowMillis));
        return new Snapshot(entries);
    }

    public static synchronized void resetForTests() {
        entries.clear();
    }

    public static Kind kindFromEventId(Identifier eventId) {
        if (eventId == null) {
            return null;
        }
        String value = eventId.toString();
        return switch (value) {
            case "bong:world_omen_pseudo_vein" -> Kind.PSEUDO_VEIN;
            case "bong:world_omen_beast_tide" -> Kind.BEAST_TIDE;
            case "bong:world_omen_realm_collapse" -> Kind.REALM_COLLAPSE;
            case "bong:world_omen_karma_backlash" -> Kind.KARMA_BACKLASH;
            default -> null;
        };
    }

    private static void upsert(Entry next) {
        entries.removeIf(entry ->
            entry.kind == next.kind && Arrays.equals(entry.origin(), next.origin())
        );
        entries.add(next);
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
