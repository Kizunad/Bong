package com.bong.client.insight;

import org.junit.jupiter.api.Test;

import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

public class ClientRequestInsightDispatcherTest {

    private static final class Capture {
        String triggerId;
        Integer idx;
        boolean called;
        void accept(String t, Integer i) { triggerId = t; idx = i; called = true; }
    }

    private static final class HeartDemonCapture {
        Integer idx;
        boolean called;
        void accept(Integer i) { idx = i; called = true; }
    }

    @Test
    void chosenResolvesChoiceIdToIdx() {
        InsightOfferViewModel offer = MockInsightOfferData.firstInduceBreakthrough();
        Capture cap = new Capture();
        var dispatcher = new ClientRequestInsightDispatcher(() -> offer, cap::accept);

        dispatcher.dispatch(InsightDecision.chosen(offer.triggerId(), "mock_choice_C3"));

        assertEquals(offer.triggerId(), cap.triggerId);
        assertEquals(Integer.valueOf(1), cap.idx); // C3 是 index 1
    }

    @Test
    void declinedSendsNullIdx() {
        InsightOfferViewModel offer = MockInsightOfferData.firstInduceBreakthrough();
        Capture cap = new Capture();
        var dispatcher = new ClientRequestInsightDispatcher(() -> offer, cap::accept);

        dispatcher.dispatch(InsightDecision.declined(offer.triggerId()));

        assertEquals(offer.triggerId(), cap.triggerId);
        assertNull(cap.idx);
    }

    @Test
    void timedOutSendsNullIdx() {
        InsightOfferViewModel offer = MockInsightOfferData.firstInduceBreakthrough();
        Capture cap = new Capture();
        var dispatcher = new ClientRequestInsightDispatcher(() -> offer, cap::accept);

        dispatcher.dispatch(InsightDecision.timedOut(offer.triggerId()));

        assertNull(cap.idx);
    }

    @Test
    void chosenWithStaleOfferDowngradesToNull() {
        Capture cap = new Capture();
        AtomicReference<InsightOfferViewModel> ref = new AtomicReference<>(null);
        var dispatcher = new ClientRequestInsightDispatcher(ref::get, cap::accept);

        dispatcher.dispatch(InsightDecision.chosen("some_offer", "mock_choice_E1"));

        assertNull(cap.idx, "missing offer snapshot should downgrade to null idx");
    }

    @Test
    void chosenWithUnknownChoiceIdDowngradesToNull() {
        InsightOfferViewModel offer = MockInsightOfferData.firstInduceBreakthrough();
        Capture cap = new Capture();
        var dispatcher = new ClientRequestInsightDispatcher(() -> offer, cap::accept);

        dispatcher.dispatch(InsightDecision.chosen(offer.triggerId(), "nonexistent_choice"));

        assertNull(cap.idx);
    }

    @Test
    void heartDemonTriggerRoutesToHeartDemonSender() {
        InsightOfferViewModel offer = MockInsightOfferData.heartDemonOffer();
        Capture insight = new Capture();
        HeartDemonCapture heartDemon = new HeartDemonCapture();
        var dispatcher = new ClientRequestInsightDispatcher(() -> offer, insight::accept, heartDemon::accept);

        dispatcher.dispatch(InsightDecision.chosen(offer.triggerId(), "heart_demon_choice_2"));

        assertEquals(Integer.valueOf(2), heartDemon.idx);
        assertEquals(false, insight.called);
    }

    @Test
    void heartDemonDeclinedSendsNullToHeartDemonSender() {
        InsightOfferViewModel offer = MockInsightOfferData.heartDemonOffer();
        Capture insight = new Capture();
        HeartDemonCapture heartDemon = new HeartDemonCapture();
        var dispatcher = new ClientRequestInsightDispatcher(() -> offer, insight::accept, heartDemon::accept);

        dispatcher.dispatch(InsightDecision.declined(offer.triggerId()));

        assertNull(heartDemon.idx);
        assertEquals(false, insight.called);
    }
}
