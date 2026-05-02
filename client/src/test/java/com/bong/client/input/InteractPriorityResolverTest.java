package com.bong.client.input;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;

public class InteractPriorityResolverTest {
    @Test
    void emptyCandidatesReturnEmpty() {
        assertFalse(InteractPriorityResolver.choose(List.of()).isPresent());
        assertFalse(InteractPriorityResolver.choose(null).isPresent());
    }

    @Test
    void highestPriorityWins() {
        InteractCandidate pickup = InteractCandidate.of(InteractIntent.PickupDroppedItem, 70, 1.0, "pickup");
        InteractCandidate trade = InteractCandidate.of(InteractIntent.TradePlayer, 90, 9.0, "trade");

        assertEquals(trade, InteractPriorityResolver.choose(List.of(pickup, trade)).orElseThrow());
    }

    @Test
    void samePriorityUsesNearestDistance() {
        InteractCandidate far = InteractCandidate.of(InteractIntent.PickupDroppedItem, 70, 9.0, "far");
        InteractCandidate near = InteractCandidate.of(InteractIntent.PickupDroppedItem, 70, 1.0, "near");

        assertEquals(near, InteractPriorityResolver.choose(List.of(far, near)).orElseThrow());
    }

    @Test
    void samePriorityAndDistanceKeepsRegistrationOrder() {
        InteractCandidate first = InteractCandidate.of(InteractIntent.PickupDroppedItem, 70, 1.0, "first");
        InteractCandidate second = InteractCandidate.of(InteractIntent.HarvestResource, 70, 1.0, "second");

        assertEquals(first, InteractPriorityResolver.choose(List.of(first, second)).orElseThrow());
    }

    @Test
    void invalidCandidateRejected() {
        assertThrows(IllegalArgumentException.class, () ->
            InteractCandidate.of(InteractIntent.PickupDroppedItem, -1, 1.0, "bad")
        );
        assertThrows(IllegalArgumentException.class, () ->
            InteractCandidate.of(InteractIntent.PickupDroppedItem, 1, Double.NaN, "bad")
        );
    }
}
