package com.bong.client.insight;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

class InsightDecisionTest {
    @Test
    void chosenRequiresChoiceId() {
        InsightDecision d = InsightDecision.chosen("offer1", "choiceA");
        assertEquals(InsightDecision.Kind.CHOSEN, d.kind());
        assertEquals("choiceA", d.chosenChoiceId());
        assertEquals("CHOSEN choiceA", d.summary());
    }

    @Test
    void declinedHasNullChoice() {
        InsightDecision d = InsightDecision.declined("offer1");
        assertEquals(InsightDecision.Kind.DECLINED, d.kind());
        assertNull(d.chosenChoiceId());
        assertEquals("DECLINED", d.summary());
    }

    @Test
    void timedOutHasNullChoice() {
        InsightDecision d = InsightDecision.timedOut("offer1");
        assertEquals(InsightDecision.Kind.TIMED_OUT, d.kind());
        assertNull(d.chosenChoiceId());
        assertEquals("TIMED_OUT", d.summary());
    }

    @Test
    void chosenWithoutChoiceIdRejected() {
        assertThrows(NullPointerException.class,
            () -> new InsightDecision("o", InsightDecision.Kind.CHOSEN, null));
    }

    @Test
    void declinedWithChoiceIdRejected() {
        assertThrows(IllegalArgumentException.class,
            () -> new InsightDecision("o", InsightDecision.Kind.DECLINED, "c"));
    }
}
