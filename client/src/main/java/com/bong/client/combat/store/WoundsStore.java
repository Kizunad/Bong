package com.bong.client.combat.store;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/**
 * Volatile snapshot of every known body-part wound, keyed by part id
 * ({@code "head" / "chest" / "left_arm" / ...}). Populated by
 * {@link com.bong.client.combat.handler.CombatEventHandler} and the wounds
 * segment of {@code combat_event}-family payloads (plan §U1).
 *
 * <p>Consumed by inspect 伤口层 bindings and by HUD near-death visual logic.
 */
public final class WoundsStore {
    /** One entry per body part. Immutable. */
    public record Wound(
        String partId,
        String kind,          // "cut" / "pierce" / "burn" / "dao_injury" / "qi_wound" / "bone_fracture" / ...
        float severity,        // 0..1
        HealingState state,    // bleeding / stanched / healing / scarred
        float infection,       // 0..1
        boolean scar,          // permanent scar marker
        long updatedAtMs       // server-stamped
    ) {
        public Wound {
            partId = partId == null ? "" : partId;
            kind = kind == null ? "" : kind;
            severity = clamp01(severity);
            state = state == null ? HealingState.BLEEDING : state;
            infection = clamp01(infection);
        }

        public int kindColor() {
            return switch (kind) {
                case "cut" -> 0xFFE04040;
                case "pierce" -> 0xFFB02020;
                case "burn" -> 0xFFFF9030;
                case "dao_injury" -> 0xFFFFD050;
                case "qi_wound" -> 0xFF60B0FF;
                case "bone_fracture" -> 0xFFD0D0D0;
                default -> 0xFF808080;
            };
        }
    }

    public enum HealingState {
        BLEEDING(0xFFE04040),
        STANCHED(0xFFE0C040),
        HEALING(0xFF60D060),
        SCARRED(0xFF303030);

        private final int color;
        HealingState(int color) { this.color = color; }
        public int color() { return color; }

        public static HealingState fromWire(String wire) {
            if (wire == null) return BLEEDING;
            return switch (wire.trim().toLowerCase(java.util.Locale.ROOT)) {
                case "stanched" -> STANCHED;
                case "healing" -> HEALING;
                case "scarred" -> SCARRED;
                default -> BLEEDING;
            };
        }
    }

    private static final WoundsStore INSTANCE = new WoundsStore();
    private volatile Map<String, Wound> snapshot = Collections.emptyMap();

    private WoundsStore() {}

    public static WoundsStore instance() { return INSTANCE; }

    public static Map<String, Wound> snapshot() { return INSTANCE.snapshot; }

    public static Wound get(String partId) {
        return INSTANCE.snapshot.get(partId == null ? "" : partId);
    }

    public static boolean hasBleedingAny() {
        for (Wound w : INSTANCE.snapshot.values()) {
            if (w.state() == HealingState.BLEEDING) return true;
        }
        return false;
    }

    public static float maxInfection() {
        float max = 0f;
        for (Wound w : INSTANCE.snapshot.values()) {
            if (w.infection() > max) max = w.infection();
        }
        return max;
    }

    public static void replace(List<Wound> wounds) {
        if (wounds == null || wounds.isEmpty()) {
            INSTANCE.snapshot = Collections.emptyMap();
            return;
        }
        Map<String, Wound> next = new LinkedHashMap<>();
        for (Wound w : wounds) {
            if (w == null) continue;
            next.put(Objects.requireNonNullElse(w.partId(), ""), w);
        }
        INSTANCE.snapshot = Collections.unmodifiableMap(next);
    }

    public static void clear() {
        INSTANCE.snapshot = Collections.emptyMap();
    }

    public static void resetForTests() {
        clear();
    }

    private static float clamp01(float v) {
        if (Float.isNaN(v)) return 0f;
        if (v < 0f) return 0f;
        if (v > 1f) return 1f;
        return v;
    }
}
