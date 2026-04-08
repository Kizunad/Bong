package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

public class NarrationToastStateTest {
    @AfterEach
    void tearDown() {
        NarrationToastState.clear();
    }

    @Test
    void showStoresActiveToastUntilExpiry() {
        NarrationToastState.show("天劫将至", 0xFF5555, 5_000L, 1_000L);

        NarrationToastState.ActiveToast activeToast = NarrationToastState.peek(5_999L);

        assertNotNull(activeToast);
        assertEquals("天劫将至", activeToast.text());
        assertEquals(0xFF5555, activeToast.color());
        assertEquals(6_000L, activeToast.expiresAtMs());
    }

    @Test
    void peekClearsExpiredToast() {
        NarrationToastState.show("新纪元已启", 0xFFAA00, 8_000L, 100L);

        assertNull(NarrationToastState.peek(8_100L));
        assertNull(NarrationToastState.peek(8_101L));
    }

    @Test
    void blankToastInputIsIgnored() {
        NarrationToastState.show("   ", 0xFFFFFF, 5_000L, 10L);

        assertNull(NarrationToastState.peek(10L));
    }
}
