package com.bong.client.botany;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyHudBootstrapTest {
    @AfterEach
    void tearDown() {
        HarvestSessionStore.resetForTests();
        SkillSetStore.resetForTests();
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
        SkillSetStore.updateEntry(
            SkillId.HERBALISM,
            new SkillSetSnapshot.Entry(4, 220L, 400L, 220L, 10, 0L, 0L)
        );

        BotanyHudBootstrap.resetOnDisconnect();

        assertTrue(HarvestSessionStore.snapshot().isEmpty());
        assertEquals(BotanySkillViewModel.defaultView(), com.bong.client.hud.BotanyHudPlanner.herbalismView());
    }
}
