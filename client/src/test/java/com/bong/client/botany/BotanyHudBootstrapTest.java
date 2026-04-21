package com.bong.client.botany;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyHudBootstrapTest {
    @AfterEach
    void tearDown() {
        HarvestSessionStore.resetForTests();
        BotanySkillStore.resetForTests();
    }

    @Test
    void resetOnDisconnectClearsBotanyStoresSafely() {
        HarvestSessionStore.replace(HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            BotanyHarvestMode.MANUAL,
            0.35,
            true,
            true,
            false,
            false,
            "请求中",
            10L
        ));
        BotanySkillStore.replace(BotanySkillViewModel.create(4, 220L, 400L, 3));

        BotanyHudBootstrap.resetOnDisconnect();

        assertTrue(HarvestSessionStore.snapshot().isEmpty());
        assertEquals(BotanySkillViewModel.defaultView(), BotanySkillStore.snapshot());
    }
}
