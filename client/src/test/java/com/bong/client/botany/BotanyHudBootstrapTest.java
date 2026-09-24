package com.bong.client.botany;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.function.BooleanSupplier;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyHudBootstrapTest {
    @AfterEach
    void tearDown() {
        HarvestSessionStore.resetForTests();
        SkillSetStore.resetForTests();
        BotanyDragState.resetForTests();
        ClientRequestSender.resetBackendForTests();
    }

    @Test
    void interactiveAutoPressDispatchesOnceAndSetsPending() {
        setInteractiveSession(true, false);
        setHerbalismLevel(3);
        List<String> sent = captureRequests();

        int consumed = pump(true, false, 1);

        assertEquals(1, consumed, "one queued press must be consumed by this pump");
        assertEquals(1, sent.size(),
            "an eligible interactive auto press must emit exactly one C2S request");
        HarvestSessionViewModel after = HarvestSessionStore.snapshot();
        assertEquals(BotanyHarvestMode.AUTO, after.mode(),
            "successful auto dispatch must update the local mode to AUTO");
        assertTrue(after.requestPending(),
            "successful auto dispatch must close the requestPending gate immediately");
        assertTrue(sent.get(0).contains("\"session_id\":\"session-botany\""),
            "auto request must preserve the active session id, payload=" + sent.get(0));
        assertTrue(sent.get(0).contains("\"mode\":\"auto\""),
            "auto request must carry AUTO mode, payload=" + sent.get(0));
    }

    @Test
    void queuedAutoPressesAreAllConsumedButOnlyFirstDispatches() {
        setInteractiveSession(true, false);
        setHerbalismLevel(3);
        List<String> sent = captureRequests();
        PressQueue queued = new PressQueue(2);

        int consumed = pump(true, false, queued);

        assertEquals(2, consumed,
            "all same-tick queued presses must be consumed instead of leaking to the next tick");
        assertEquals(0, pump(true, false, queued),
            "the same input queue must be empty after the first pump");
        assertEquals(1, sent.size(),
            "requestPending must block the second queued press in the same logical tick");
        assertTrue(HarvestSessionStore.snapshot().requestPending(),
            "the first accepted request must leave the store pending while the second is drained");
    }

    @Test
    void nonInteractivePressesAreDrainedAndDoNotReplayAfterSessionOpens() {
        HarvestSessionStore.replace(HarvestSessionViewModel.empty());
        List<String> sent = captureRequests();
        PressQueue queued = new PressQueue(2);

        assertEquals(2, pump(false, false, queued),
            "non-interactive path must discard every queued auto press");
        setInteractiveSession(true, false);
        setHerbalismLevel(3);
        assertEquals(0, pump(true, false, queued),
            "opening a session without a new press must not manufacture an auto request");

        assertTrue(sent.isEmpty(),
            "discarded non-interactive presses must never be replayed as C2S requests");
        assertFalse(HarvestSessionStore.snapshot().requestPending(),
            "draining a blocked key queue must not mutate the later session state");
    }

    @Test
    void screenOpenPressesAreDrainedWithoutDispatch() {
        setInteractiveSession(true, false);
        setHerbalismLevel(3);
        List<String> sent = captureRequests();
        PressQueue queued = new PressQueue(2);

        assertEquals(2, pump(true, true, queued),
            "screen-gated path must drain all auto presses");
        assertEquals(0, pump(true, false, queued),
            "no new press exists after the screen-gated queue was drained");

        assertTrue(sent.isEmpty(),
            "a current screen must suppress auto dispatch rather than defer it");
        assertFalse(HarvestSessionStore.snapshot().requestPending(),
            "screen-gated input must not change the request state");
    }

    @Test
    void autoSelectableFalseDoesNotDispatchButConsumesPresses() {
        setInteractiveSession(false, false);
        setHerbalismLevel(10);
        List<String> sent = captureRequests();
        HarvestSessionViewModel before = HarvestSessionStore.snapshot();
        PressQueue queued = new PressQueue(2);

        assertEquals(2, pump(true, false, queued),
            "an unavailable auto action must still consume every queued press");
        assertEquals(0, pump(true, false, queued),
            "autoSelectable rejection must not leave presses queued for a later tick");

        assertTrue(sent.isEmpty(),
            "autoSelectable=false must reject dispatch even when herbalism is unlocked");
        assertEquals(before, HarvestSessionStore.snapshot(),
            "rejected auto selection must leave the session state unchanged");
    }

    @Test
    void herbalismBelowAutoUnlockDoesNotDispatchButConsumesPresses() {
        setInteractiveSession(true, false);
        setHerbalismLevel(2);
        List<String> sent = captureRequests();
        HarvestSessionViewModel before = HarvestSessionStore.snapshot();
        PressQueue queued = new PressQueue(2);

        assertEquals(2, pump(true, false, queued),
            "locked herbalism level must not leave queued presses for a later tick");
        assertEquals(0, pump(true, false, queued),
            "skill-gated rejection must leave no auto presses for a later tick");

        assertTrue(sent.isEmpty(),
            "herbalism below level three must reject auto dispatch");
        assertEquals(before, HarvestSessionStore.snapshot(),
            "skill-gated auto input must not mutate the session state");
    }

    @Test
    void emptySessionDoesNotDispatchOrMutateWhenPressed() {
        HarvestSessionStore.replace(HarvestSessionViewModel.empty());
        List<String> sent = captureRequests();
        PressQueue queued = new PressQueue(2);

        assertEquals(2, pump(false, false, queued),
            "an empty session is blocked and must drain queued auto presses");
        assertEquals(0, pump(false, false, queued),
            "empty-session drain must leave no queued input behind");

        assertTrue(sent.isEmpty(), "empty session must fail closed without a C2S request");
        assertEquals(HarvestSessionViewModel.empty(), HarvestSessionStore.snapshot(),
            "empty session rejection must not create or mutate session state");
    }

    @Test
    void interruptedSessionDoesNotReplayPressesAfterRecovery() {
        setInteractiveSession(true, false);
        HarvestSessionStore.requestMode(BotanyHarvestMode.MANUAL, 15L);
        HarvestSessionStore.interruptLocally("受击打断", 20L);
        List<String> sent = captureRequests();
        PressQueue queued = new PressQueue(1);

        assertEquals(1, pump(false, false, queued),
            "interrupted session must drain presses while it is unavailable");
        setInteractiveSession(true, false);
        setHerbalismLevel(3);
        assertEquals(0, pump(true, false, queued),
            "recovery without a new key press must not replay the interrupted input");

        assertTrue(sent.isEmpty(), "interrupted input must not dispatch after recovery");
        assertFalse(HarvestSessionStore.snapshot().requestPending(),
            "draining an interrupted queue must not mark the recovered session pending");
    }

    @Test
    void livePendingGateBlocksSecondPressAfterFirstDispatch() {
        setInteractiveSession(true, false);
        setHerbalismLevel(3);
        List<String> sent = captureRequests();

        pump(true, false, 2);

        assertEquals(1, sent.size(),
            "the second press must observe the store's false-to-true pending transition");
        assertTrue(HarvestSessionStore.snapshot().requestPending(),
            "live gate test must finish with requestPending=true after the first dispatch");
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

    private static int pump(boolean interactive, boolean screenOpen, int pressCount) {
        return pump(interactive, screenOpen, new PressQueue(pressCount));
    }

    private static int pump(boolean interactive, boolean screenOpen, PressQueue queued) {
        return BotanyHudBootstrap.pumpAutoHarvestPresses(
            interactive,
            screenOpen,
            queued
        );
    }

    private static List<String> captureRequests() {
        List<String> sent = new ArrayList<>();
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new String(payload, StandardCharsets.UTF_8))
        );
        return sent;
    }

    private static void setInteractiveSession(boolean autoSelectable, boolean requestPending) {
        HarvestSessionStore.replace(HarvestSessionViewModel.create(
            "session-botany",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            null,
            0.35,
            autoSelectable,
            requestPending,
            false,
            false,
            "",
            10L
        ));
    }

    private static void setHerbalismLevel(int level) {
        SkillSetStore.updateEntry(
            SkillId.HERBALISM,
            new SkillSetSnapshot.Entry(level, 0L, 100L, 0L, 10, 0L, 0L)
        );
    }

    private static final class PressQueue implements BooleanSupplier {
        private int remaining;

        private PressQueue(int remaining) {
            this.remaining = remaining;
        }

        @Override
        public boolean getAsBoolean() {
            if (remaining == 0) {
                return false;
            }
            remaining--;
            return true;
        }
    }
}
