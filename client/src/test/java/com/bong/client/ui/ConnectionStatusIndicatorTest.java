package com.bong.client.ui;

import com.bong.client.hud.BongToast;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ConnectionStatusIndicatorTest {
    @AfterEach
    void resetStore() {
        ClientConnectionStatusStore.resetForTests();
        BongToast.resetForTests();
    }

    @Test
    void successivePhysicalSessionsReceiveDistinctSequenceTokens() {
        Object firstHandler = new Object();
        Object secondHandler = new Object();

        ClientConnectionStatusStore.SessionToken firstToken =
            ClientConnectionStatusStore.initializeSession(firstHandler);
        ClientConnectionStatusStore.SessionToken repeatedFirstToken =
            ClientConnectionStatusStore.initializeSession(firstHandler);
        ClientConnectionStatusStore.SessionToken secondToken =
            ClientConnectionStatusStore.initializeSession(secondHandler);

        assertSame(firstToken, repeatedFirstToken,
            "同一物理 handler 的重复 INIT 必须保持原 session token identity");
        assertEquals("SessionToken[1]", firstToken.toString(),
            "reset 后首条物理连接必须获得首个可观察序号");
        assertEquals("SessionToken[2]", secondToken.toString(),
            "后继物理连接必须推进可观察序号，而非重复分配 SessionToken[1]");
    }

    @Test
    void connection_indicator_green_on_connect() {
        ConnectionStatusIndicator.Snapshot snapshot = ConnectionStatusIndicator.evaluate(true, 37L, 0L, 100L);

        assertEquals(ConnectionStatusIndicator.Status.GREEN, snapshot.status());
        assertEquals(ConnectionStatusIndicator.GREEN, snapshot.color());
        assertTrue(snapshot.tooltip().contains("37ms"));
    }

    @Test
    void connection_indicator_does_not_fake_unknown_latency() {
        ConnectionStatusIndicator.Snapshot snapshot = ConnectionStatusIndicator.evaluate(
            true,
            ConnectionStatusIndicator.UNKNOWN_LATENCY_MS,
            0L,
            100L
        );

        assertEquals(ConnectionStatusIndicator.Status.GREEN, snapshot.status());
        assertTrue(snapshot.tooltip().contains("延迟 --"));
        assertEquals(ConnectionStatusIndicator.UNKNOWN_LATENCY_MS, snapshot.latencyMs());
    }

    @Test
    void connection_indicator_yellow_on_delay() {
        ConnectionStatusIndicator.Snapshot snapshot = ConnectionStatusIndicator.evaluate(true, 42L, 0L, 6_000L);

        assertEquals(ConnectionStatusIndicator.Status.YELLOW, snapshot.status());
        assertEquals(ConnectionStatusIndicator.YELLOW, snapshot.color());
    }

    @Test
    void connection_indicator_red_on_disconnect() {
        ConnectionStatusIndicator.Snapshot snapshot = ConnectionStatusIndicator.evaluate(false, 0L, 11_000L, Long.MAX_VALUE);

        assertEquals(ConnectionStatusIndicator.Status.RED, snapshot.status());
        assertEquals(ConnectionStatusIndicator.RED, snapshot.color());
        assertTrue(snapshot.tooltip().contains("断开 11s"));
    }


    @Test
    void disconnect_toast_once() {
        Object handler = connectAt(500L);
        assertTrue(ClientConnectionStatusStore.invalidateSession(handler, 1_000L));
        ClientConnectionStatusStore.tick(12_000L);
        long firstExpiry = BongToast.current(12_001L).expiresAtMillis();

        ClientConnectionStatusStore.tick(12_500L);
        long secondExpiry = BongToast.current(12_501L).expiresAtMillis();

        assertFalse(BongToast.current(12_001L).isEmpty());
        assertEquals(firstExpiry, secondExpiry);
    }

    @Test
    void reconnect_toast_after_red() {
        Object oldHandler = connectAt(500L);
        assertTrue(ClientConnectionStatusStore.invalidateSession(oldHandler, 1_000L));
        ClientConnectionStatusStore.tick(12_000L);
        connectAt(13_000L);
        ClientConnectionStatusStore.tick(13_000L);

        assertEquals("天道重注", BongToast.current(13_001L).text().getString());
    }

    @Test
    void connection_status_uses_measuring_time_without_breaking_toast_wall_time() {
        Object handler = connectAt(500L);
        assertTrue(ClientConnectionStatusStore.invalidateSession(handler, 1_000L));
        ClientConnectionStatusStore.tick(12_000L, 50_000L);

        assertFalse(BongToast.current(50_001L).isEmpty());
        assertEquals(53_000L, BongToast.current(50_001L).expiresAtMillis());
    }

    private static Object connectAt(long nowMs) {
        Object handler = new Object();
        ClientConnectionStatusStore.initializeSession(handler);
        assertTrue(
            ClientConnectionStatusStore.activateSession(handler, nowMs),
            "JOIN 必须激活 INIT 已分配的物理连接 token"
        );
        return handler;
    }
}
