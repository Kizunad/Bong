package com.bong.client.ui;

import com.bong.client.hud.BongToast;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ConnectionStatusIndicatorTest {
    @AfterEach
    void resetStore() {
        ClientConnectionStatusStore.resetForTests();
        BongToast.resetForTests();
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
    void indicator_builds_bottom_right_dot() {
        List<HudRenderCommand> commands = ConnectionStatusIndicator.buildCommands(
            ConnectionStatusIndicator.evaluate(true, 42L, 0L, 0L),
            320,
            180
        );

        assertEquals(1, commands.size());
        assertEquals(HudRenderLayer.CONNECTION_STATUS, commands.get(0).layer());
        assertTrue(commands.get(0).isRect());
        assertEquals(302, commands.get(0).x());
        assertEquals(152, commands.get(0).y());
    }

    @Test
    void disconnect_toast_once() {
        ClientConnectionStatusStore.markDisconnected(1_000L);
        ClientConnectionStatusStore.tick(12_000L);
        long firstExpiry = BongToast.current(12_001L).expiresAtMillis();

        ClientConnectionStatusStore.tick(12_500L);
        long secondExpiry = BongToast.current(12_501L).expiresAtMillis();

        assertFalse(BongToast.current(12_001L).isEmpty());
        assertEquals(firstExpiry, secondExpiry);
    }

    @Test
    void reconnect_toast_after_red() {
        ClientConnectionStatusStore.markDisconnected(1_000L);
        ClientConnectionStatusStore.tick(12_000L);
        ClientConnectionStatusStore.markConnected(13_000L);
        ClientConnectionStatusStore.tick(13_000L);

        assertEquals("天道重注", BongToast.current(13_001L).text().getString());
    }
}
