package com.bong.client.insight;

import net.minecraft.client.gui.screen.Screen;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;

class InsightOfferScreenBootstrapTest {
    @AfterEach
    void cleanup() {
        InsightOfferStore.resetForTests();
    }

    @Test
    void preOpenExpirySettlesOnceWithoutConstructingScreen() {
        List<InsightDecision> decisions = new ArrayList<>();
        List<Screen> screens = new ArrayList<>();
        InsightOfferViewModel expired = InsightOfferFixtures.firstInduceBreakthrough(1_000L);
        InsightOfferStore.setDispatcher(decisions::add);
        InsightOfferStore.replace(expired);

        InsightOfferScreenBootstrap.applyStoreChange(null, screens::add, expired, () -> 1_000L);

        assertEquals(List.of(InsightDecision.timedOut(expired.triggerId())), decisions,
            "pre-open expiry must dispatch exactly one timeout decision");
        assertEquals(List.of(), screens, "pre-open expiry must not construct a screen");
        assertNull(InsightOfferStore.snapshot(), "expired current offer must be cleared after settlement");
        assertEquals(InsightOfferStore.TerminalCause.TIMEOUT,
            InsightOfferStore.terminalCauseForTests(expired.triggerId()),
            "pre-open timeout must claim the shared trigger terminal cause");

        InsightOfferScreenBootstrap.applyStoreChange(null, screens::add, expired, () -> 2_000L);

        assertEquals(1, decisions.size(), "repeated expiry observation must not resend timeout");
        assertEquals(List.of(), screens, "repeated expiry observation must still not create a screen");
    }

    @Test
    void replacementSettlesOutgoingBeforeOpeningReplacementAndPreservesNewSnapshot() {
        List<InsightDecision> decisions = new ArrayList<>();
        List<Screen> screens = new ArrayList<>();
        InsightOfferViewModel outgoing = InsightOfferFixtures.firstInduceBreakthrough(10_000L);
        InsightOfferViewModel replacement = new InsightOfferViewModel(
            "replacement-trigger",
            outgoing.triggerLabel(),
            outgoing.realmLabel(),
            outgoing.composure(),
            outgoing.quotaRemaining(),
            outgoing.quotaTotal(),
            10_000L,
            outgoing.choices()
        );
        InsightOfferStore.setDispatcher(decisions::add);
        InsightOfferStore.replace(outgoing);
        InsightOfferScreen current = new InsightOfferScreen(outgoing);
        InsightOfferStore.replace(replacement);

        InsightOfferScreenBootstrap.applyStoreChange(current, screens::add, replacement, () -> 1_000L);

        assertEquals(List.of(InsightDecision.declined(outgoing.triggerId())), decisions,
            "replacement must decline the outgoing trigger before opening the replacement");
        assertEquals(1, screens.size(), "replacement must create exactly one screen");
        InsightOfferScreen opened = assertInstanceOf(InsightOfferScreen.class, screens.get(0));
        assertEquals(replacement.triggerId(), opened.offer().triggerId(),
            "the only opened screen must own the replacement trigger");
        assertSame(replacement, InsightOfferStore.snapshot(),
            "settling the stale outgoing object must not clear the replacement snapshot");
        assertEquals(InsightOfferStore.TerminalCause.REPLACED_BY_DIFFERENT_OFFER,
            InsightOfferStore.terminalCauseForTests(outgoing.triggerId()),
            "outgoing trigger must retain the replacement terminal cause");
    }

    @Test
    void currentScreenCancellationSettlesOnceAndRemovedDoesNotRetry() {
        List<InsightDecision> decisions = new ArrayList<>();
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough(10_000L);
        InsightOfferStore.setDispatcher(decisions::add);
        InsightOfferStore.replace(offer);
        InsightOfferScreen screen = new InsightOfferScreen(offer);

        screen.onCurrentScreenCancelled();
        screen.removed();

        assertEquals(List.of(InsightDecision.declined(offer.triggerId())), decisions,
            "transition cancellation and later removal must emit one decision total");
        assertEquals(InsightOfferStore.TerminalCause.REPLACED_BY_DIFFERENT_OFFER,
            InsightOfferStore.terminalCauseForTests(offer.triggerId()),
            "first cancellation cause must stay immutable after removal");
        assertNull(InsightOfferStore.snapshot(), "current offer must be cleared after cancellation settlement");
    }
}
