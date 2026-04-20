package com.bong.client.botany;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class HarvestSessionStoreTest {
    @AfterEach
    void tearDown() {
        HarvestSessionStore.resetForTests();
    }

    @Test
    void requestModeMarksSessionPendingLocally() {
        HarvestSessionStore.replace(HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            null,
            0.0,
            true,
            false,
            false,
            false,
            "晨露尚存",
            10L
        ));

        HarvestSessionStore.requestMode(BotanyHarvestMode.MANUAL, 20L);

        HarvestSessionViewModel snapshot = HarvestSessionStore.snapshot();
        assertEquals(BotanyHarvestMode.MANUAL, snapshot.mode());
        assertTrue(snapshot.requestPending());
        assertTrue(snapshot.interactive());
    }

    @Test
    void clearOnDisconnectResetsActiveSessionSafely() {
        HarvestSessionStore.replace(HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            BotanyHarvestMode.AUTO,
            0.5,
            true,
            false,
            false,
            false,
            "灵气稳定",
            10L
        ));

        HarvestSessionStore.clearOnDisconnect();

        assertTrue(HarvestSessionStore.snapshot().isEmpty());
    }

    @Test
    void localInterruptRequiresChosenMode() {
        HarvestSessionStore.replace(HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            null,
            0.25,
            true,
            false,
            false,
            false,
            "等待选择",
            10L
        ));

        HarvestSessionStore.interruptLocally("移动打断", 15L);
        assertFalse(HarvestSessionStore.snapshot().interrupted());

        HarvestSessionStore.requestMode(BotanyHarvestMode.MANUAL, 20L);
        HarvestSessionStore.interruptLocally("移动打断", 30L);
        assertTrue(HarvestSessionStore.snapshot().interrupted());
        assertEquals("移动打断", HarvestSessionStore.snapshot().detail());
    }
}
