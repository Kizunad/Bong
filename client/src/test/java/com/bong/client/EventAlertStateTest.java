package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

public class EventAlertStateTest {
    @AfterEach
    void tearDown() {
        EventAlertState.clear();
    }

    @Test
    void showStoresAlertUntilExpiry() {
        EventAlertState.show("天劫预警", 5_000L, 100L);

        EventAlertState.ActiveAlert alert = EventAlertState.peek(5_099L);
        assertNotNull(alert);
        assertEquals("天劫预警", alert.message());
        assertEquals(5_100L, alert.expiresAtMs());
        assertNull(EventAlertState.peek(5_100L));
    }

    @Test
    void invalidInputsAreIgnored() {
        EventAlertState.show("   ", 5_000L, 10L);
        assertNull(EventAlertState.peek(10L));
    }
}
