package com.bong.client.botany;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyHudBootstrapTest {
    @AfterEach
    void tearDown() {
        HarvestSessionStore.resetForTests();
        SkillSetStore.resetForTests();
        BotanyDragState.resetForTests();
    }

    @Test
    void disconnectCleanupClearsDragRuntimeButLeavesStoresToRegistry() {
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
        BotanyDragState.recordRenderedBounds(100, 100, 280, 200);
        assertTrue(BotanyDragState.onLeftButton(1, 150.0, 150.0));
        BotanyDragState.tickDrag(180.0, 190.0);

        BotanyHudBootstrap.clearOnDisconnect();
        BotanyHudBootstrap.clearOnDisconnect();

        assertFalse(BotanyDragState.isDragging());
        assertEquals(0, BotanyDragState.deltaX());
        assertEquals(0, BotanyDragState.deltaY());
        assertFalse(HarvestSessionStore.snapshot().isEmpty(),
            "botany adjunct cleaner must leave HarvestSessionStore to the central registry");
        assertEquals(4, SkillSetStore.snapshot().get(SkillId.HERBALISM).effectiveLv(),
            "botany adjunct cleaner must preserve registry-owned skill data");
    }

    @Test
    void blockedAutoPressesDrainWithoutReplayAndLaterFreshPressDispatches() {
        AtomicInteger queued = new AtomicInteger(2);
        AtomicInteger dispatched = new AtomicInteger();

        assertEquals(0, BotanyHudBootstrap.consumeAutoPresses(
            false,
            () -> queued.getAndUpdate(value -> Math.max(0, value - 1)) > 0,
            dispatched::incrementAndGet
        ), "blocked input must dispatch nothing");
        assertEquals(0, queued.get(), "blocked input must drain the complete physical press queue");
        assertEquals(0, dispatched.get(), "blocked input must not later replay as a mode request");

        assertEquals(0, BotanyHudBootstrap.consumeAutoPresses(
            true,
            () -> queued.getAndUpdate(value -> Math.max(0, value - 1)) > 0,
            dispatched::incrementAndGet
        ), "opening the gate after draining must not replay old presses");
        assertEquals(0, dispatched.get(), "old blocked presses must remain discarded after the gate opens");

        queued.incrementAndGet();
        assertEquals(1, BotanyHudBootstrap.consumeAutoPresses(
            true,
            () -> queued.getAndUpdate(value -> Math.max(0, value - 1)) > 0,
            dispatched::incrementAndGet
        ), "a newly pressed AUTO key after the gate opens must still dispatch normally");
        assertEquals(1, dispatched.get(), "only the fresh allowed press may dispatch");
    }
}
