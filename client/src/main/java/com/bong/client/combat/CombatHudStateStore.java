package com.bong.client.combat;

/**
 * Volatile snapshot store for {@link CombatHudState} — consumed by the HUD
 * planner, updated by the network handler for channel
 * {@code bong:combat/hud_state} (§11.4).
 */
public final class CombatHudStateStore {
    private static volatile CombatHudState snapshot = CombatHudState.empty();

    private CombatHudStateStore() {
    }

    public static CombatHudState snapshot() {
        return snapshot;
    }

    public static void replace(CombatHudState next) {
        snapshot = next == null ? CombatHudState.empty() : next;
    }

    public static void clear() {
        snapshot = CombatHudState.empty();
    }

    public static void resetForTests() {
        clear();
    }
}
