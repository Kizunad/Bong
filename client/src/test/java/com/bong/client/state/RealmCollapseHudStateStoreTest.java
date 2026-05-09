package com.bong.client.state;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class RealmCollapseHudStateStoreTest {
    @AfterEach
    void clearStaticState() {
        RealmCollapseHudStateStore.resetForTests();
    }

    @Test
    void clearOnDisconnectResetsActiveAlertSafely() {
        RealmCollapseHudStateStore.replace(
            RealmCollapseHudState.create("north_wastes", "区域 north_wastes · 立即离开边界", 1_000L, 12_000)
        );
        assertFalse(RealmCollapseHudStateStore.snapshot().isEmpty(), "alert should be live before disconnect");

        RealmCollapseHudStateStore.clearOnDisconnect();

        assertTrue(
            RealmCollapseHudStateStore.snapshot().isEmpty(),
            "disconnect must wipe the static volatile snapshot so the next session starts clean (旧 server 的 evac 倒计时不能跨 reconnect 续命)"
        );
    }

    @Test
    void clearOnDisconnectIsIdempotentOnEmptyState() {
        RealmCollapseHudStateStore.resetForTests();
        assertTrue(RealmCollapseHudStateStore.snapshot().isEmpty());

        RealmCollapseHudStateStore.clearOnDisconnect();

        assertTrue(RealmCollapseHudStateStore.snapshot().isEmpty());
    }
}
