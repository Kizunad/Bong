package com.bong.client.combat.store;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;

/**
 * Full snapshot of all active status effects (plan §U2, §2.5).
 *
 * <p>Populated by {@link com.bong.client.combat.handler.StatusSnapshotHandler}.
 * Sorted by {@link #rank} for the top-8 HUD status bar; inspect status panel
 * groups by {@link Effect#kind()}.
 */
public final class StatusEffectStore {
    public enum Kind {
        DOT("dot"),
        CONTROL("control"),
        BUFF("buff"),
        DEBUFF("debuff"),
        UNKNOWN("unknown");

        private final String wireName;
        Kind(String wireName) { this.wireName = wireName; }
        public String wireName() { return wireName; }

        public static Kind fromWire(String wire) {
            if (wire == null) return UNKNOWN;
            return switch (wire.trim().toLowerCase(java.util.Locale.ROOT)) {
                case "dot" -> DOT;
                case "control" -> CONTROL;
                case "buff" -> BUFF;
                case "debuff" -> DEBUFF;
                default -> UNKNOWN;
            };
        }
    }

    public record Effect(
        String id,
        String displayName,
        Kind kind,
        int stacks,
        long remainingMs,
        int sourceColor,
        String sourceLabel,
        int dispelDifficulty  // 0..5, for tooltip
    ) {
        public Effect {
            id = id == null ? "" : id;
            displayName = displayName == null ? "" : displayName;
            kind = kind == null ? Kind.UNKNOWN : kind;
            stacks = Math.max(0, stacks);
            remainingMs = Math.max(0L, remainingMs);
            dispelDifficulty = Math.max(0, Math.min(5, dispelDifficulty));
        }
    }

    public static final int TOP_BAR_LIMIT = 8;

    private static final StatusEffectStore INSTANCE = new StatusEffectStore();
    private volatile List<Effect> snapshot = Collections.emptyList();

    private StatusEffectStore() {}

    public static StatusEffectStore instance() { return INSTANCE; }

    public static List<Effect> snapshot() { return INSTANCE.snapshot; }

    public static List<Effect> topBar() {
        List<Effect> all = INSTANCE.snapshot;
        if (all.isEmpty()) return Collections.emptyList();
        List<Effect> sorted = new ArrayList<>(all);
        sorted.sort(Comparator
            .comparingInt((Effect e) -> rank(e.kind()))
            .thenComparing((Effect e) -> e.remainingMs()));
        if (sorted.size() > TOP_BAR_LIMIT) {
            return Collections.unmodifiableList(sorted.subList(0, TOP_BAR_LIMIT));
        }
        return Collections.unmodifiableList(sorted);
    }

    /** Priority ordering (lower = higher priority): DoT > Control > Debuff > Buff > Unknown. */
    public static int rank(Kind kind) {
        return switch (kind) {
            case DOT -> 0;
            case CONTROL -> 1;
            case DEBUFF -> 2;
            case BUFF -> 3;
            case UNKNOWN -> 4;
        };
    }

    public static void replace(List<Effect> effects) {
        if (effects == null || effects.isEmpty()) {
            INSTANCE.snapshot = Collections.emptyList();
            return;
        }
        List<Effect> cleaned = new ArrayList<>(effects.size());
        for (Effect e : effects) {
            if (e == null) continue;
            cleaned.add(e);
        }
        INSTANCE.snapshot = Collections.unmodifiableList(cleaned);
    }

    public static void clear() {
        INSTANCE.snapshot = Collections.emptyList();
    }

    public static void resetForTests() {
        clear();
    }
}
