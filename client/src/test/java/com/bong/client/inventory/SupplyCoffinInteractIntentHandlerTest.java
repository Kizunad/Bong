package com.bong.client.inventory;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SupplyCoffinInteractIntentHandlerTest {
    @Test
    void candidateReturnsEmptyWhenClientIsNull() {
        Optional<InteractCandidate> result = new SupplyCoffinInteractIntentHandler().candidate(null);
        assertFalse(result.isPresent(), "candidate(null) must return Optional.empty()");
    }

    @Test
    void candidateEntityIdParsesValidLabel() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer,
            10,
            1.0,
            "supply_coffin:42"
        );
        assertEquals(42, SupplyCoffinInteractIntentHandler.candidateEntityId(candidate));
    }

    @Test
    void candidateEntityIdRejectsWrongPrefix() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer,
            10,
            1.0,
            "storage_crate:42"
        );
        assertEquals(-1, SupplyCoffinInteractIntentHandler.candidateEntityId(candidate));
    }

    @Test
    void candidateEntityIdRejectsNullCandidate() {
        assertEquals(-1, SupplyCoffinInteractIntentHandler.candidateEntityId(null));
    }

    @Test
    void candidateEntityIdRejectsNonNumericSuffix() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer,
            10,
            1.0,
            "supply_coffin:not_a_number"
        );
        assertEquals(-1, SupplyCoffinInteractIntentHandler.candidateEntityId(candidate));
    }

    // ============= isWithinInteractRange =============
    //
    // Pin tests locking the supply-coffin gate to the server-side
    // OPEN_RANGE_BLOCKS + OPEN_RANGE_TOLERANCE = 4.5 boundary
    // (server/src/supply_coffin/interact.rs), preventing regression to the
    // client=5.0/server=4.5 drift from plan-bughunt-entity-interact-range-desync-v1.
    // This path has no client-visible reject feedback, so a phantom-interact window
    // here feels like a dead key press, not just "too far" — the boundary must be exact.

    @Test
    void isWithinInteractRangeAcceptsExactlyAtBoundary() {
        double boundaryDistSq = 4.5 * 4.5;
        assertTrue(
            SupplyCoffinInteractIntentHandler.isWithinInteractRange(boundaryDistSq),
            "distSq exactly at 4.5^2 (server's OPEN_RANGE_BLOCKS + OPEN_RANGE_TOLERANCE) must be a candidate"
        );
    }

    @Test
    void isWithinInteractRangeRejectsJustBeyondBoundary() {
        double justBeyondDistSq = 4.51 * 4.51;
        assertFalse(
            SupplyCoffinInteractIntentHandler.isWithinInteractRange(justBeyondDistSq),
            "distSq just beyond 4.5 blocks must not be a candidate — server silently rejects "
                + "here with no chat feedback, so this drift felt like a dead key press before "
                + "plan-bughunt-entity-interact-range-desync-v1 closed it (old client gate was 5.0)"
        );
    }

    @Test
    void isWithinInteractRangeRejectsOldFivePointZeroWindow() {
        double phantomWindowDistSq = 4.8 * 4.8;
        assertFalse(
            SupplyCoffinInteractIntentHandler.isWithinInteractRange(phantomWindowDistSq),
            "4.8 blocks is inside the old (incorrect) 5.0 client radius but outside the "
                + "true 4.5 server radius — must be rejected now"
        );
    }

    @Test
    void isWithinInteractRangeAcceptsZeroDistance() {
        assertTrue(SupplyCoffinInteractIntentHandler.isWithinInteractRange(0.0));
    }
}
