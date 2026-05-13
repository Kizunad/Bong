package com.bong.client.insight;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

class InsightOfferStoreTest {
    @AfterEach
    void cleanup() {
        InsightOfferStore.resetForTests();
    }

    @Test
    void replaceNotifiesListeners() {
        List<InsightOfferViewModel> notified = new ArrayList<>();
        InsightOfferStore.addListener(notified::add);

        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();
        InsightOfferStore.replace(offer);

        assertEquals(1, notified.size());
        assertEquals(offer, notified.get(0));
        assertEquals(offer, InsightOfferStore.snapshot());
    }

    @Test
    void submitSendsDecisionThroughDispatcherAndClearsSlot() {
        List<InsightDecision> dispatched = new ArrayList<>();
        InsightOfferStore.setDispatcher(dispatched::add);
        InsightOfferViewModel offer = InsightOfferFixtures.firstInduceBreakthrough();

        InsightOfferStore.replace(offer);
        InsightOfferStore.submit(InsightDecision.chosen(offer.triggerId(), offer.choices().get(0).choiceId()));

        assertEquals(1, dispatched.size());
        assertEquals("CHOSEN fixture_choice_E1", dispatched.get(0).summary());
        assertNull(InsightOfferStore.snapshot());
    }

    @Test
    void resetClearsSnapshotAndDispatcherAndListeners() {
        List<InsightOfferViewModel> notified = new ArrayList<>();
        InsightOfferStore.addListener(notified::add);
        InsightOfferStore.replace(InsightOfferFixtures.firstInduceBreakthrough());

        InsightOfferStore.resetForTests();

        assertNull(InsightOfferStore.snapshot());
        assertEquals(InsightChoiceDispatcher.LOGGING, InsightOfferStore.dispatcher());

        // Listener should be detached too: replacing again must not notify
        InsightOfferStore.replace(InsightOfferFixtures.firstInduceBreakthrough());
        assertEquals(1, notified.size()); // only the initial replace before reset
    }
}
