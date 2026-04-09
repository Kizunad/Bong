package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongNetworkHandlerTest {
    @AfterEach
    void resetUnknownTypeLogCache() {
        BongNetworkHandler.resetUnknownTypeLogTimesForTests();
    }

    @Test
    void firstUnknownTypeIsLoggable() {
        assertTrue(BongNetworkHandler.shouldLogNoOp("mystery_signal", 1_000L));
    }

    @Test
    void repeatedUnknownTypeIsThrottledWithinWindow() {
        assertTrue(BongNetworkHandler.shouldLogNoOp("mystery_signal", 1_000L));
        assertFalse(BongNetworkHandler.shouldLogNoOp("mystery_signal", 1_001L));
        assertTrue(BongNetworkHandler.shouldLogNoOp("mystery_signal", 31_001L));
    }

    @Test
    void unknownTypeThrottleCacheStaysBounded() {
        int cacheLimit = BongNetworkHandler.unknownTypeLogCacheLimitForTests();

        for (int index = 0; index < cacheLimit * 4; index++) {
            assertTrue(BongNetworkHandler.shouldLogNoOp("mystery_signal_" + index, 1_000L));
        }

        assertEquals(cacheLimit, BongNetworkHandler.unknownTypeLogCacheSizeForTests());
    }
}
