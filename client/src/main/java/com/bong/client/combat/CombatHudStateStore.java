package com.bong.client.combat;

/**
 * Volatile snapshot store for {@link CombatHudState} — consumed by the HUD
 * planner, updated by the network handler for channel
 * {@code bong:combat/hud_state} (§11.4).
 */
public final class CombatHudStateStore {
    private static volatile StoredSnapshot stored = new StoredSnapshot(CombatHudState.empty(), false);

    private CombatHudStateStore() {
    }

    public static CombatHudState snapshot() {
        return stored.state();
    }

    public static void replace(CombatHudState next) {
        StoredSnapshot current = stored;
        stored = new StoredSnapshot(next == null ? CombatHudState.empty() : next, current.authoritative());
    }

    /** 只允许网络 handler 调用，标记已经收到至少一帧合法服务端权威快照。 */
    public static void replaceAuthoritative(CombatHudState next) {
        stored = new StoredSnapshot(next == null ? CombatHudState.empty() : next, next != null);
    }

    /** 返回最新权威快照；尚未收到合法服务端帧时返回 null，供策略层 fail closed。 */
    public static CombatHudState authoritativeSnapshot() {
        StoredSnapshot current = stored;
        return current.authoritative() ? current.state() : null;
    }

    public static void clear() {
        stored = new StoredSnapshot(CombatHudState.empty(), false);
    }

    public static void clearOnDisconnect() {
        clear();
    }

    public static void resetForTests() {
        clear();
    }

    private record StoredSnapshot(CombatHudState state, boolean authoritative) {
    }
}
