package com.bong.client.hud;

public final class BongHudStateStore {
    private static volatile BongHudStateSnapshot snapshot = BongHudStateSnapshot.empty();

    private BongHudStateStore() {
    }

    public static BongHudStateSnapshot snapshot() {
        return snapshot;
    }

    public static void replace(BongHudStateSnapshot nextSnapshot) {
        snapshot = nextSnapshot == null ? BongHudStateSnapshot.empty() : nextSnapshot;
    }

    /**
     * plan-bughunt-hud-state-session-reset — 断线/切服时重置生产 HUD snapshot。
     *
     * <p>{@code snapshot} 是跨 session 存活的 static，不显式 reset 会让上一 server
     * 写入的 {@code zoneState}（区域 overlay/atmosphere）与 {@code visualEffectState}
     * （HUD tint、相机偏移、FOV）残留到下一个 session，直到新服发来首个 {@code zone_info}
     * 或旧 visual effect 自然过期为止。
     */
    public static void clear() {
        snapshot = BongHudStateSnapshot.empty();
    }

    /** Clears session-scoped state while preserving process-lifetime wiring. */
    public static void clearOnDisconnect() {
        clear();
    }
}
