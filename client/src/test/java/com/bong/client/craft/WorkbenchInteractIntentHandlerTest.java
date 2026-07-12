package com.bong.client.craft;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.EnumSource;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class WorkbenchInteractIntentHandlerTest {
    @Test
    void workbenchKindIsRecognised() {
        assertTrue(WorkbenchInteractIntentHandler.isWorkbenchKind(BongEntityModelKind.WORKBENCH));
    }

    @ParameterizedTest(name = "{0} should not be a workbench kind")
    @EnumSource(value = BongEntityModelKind.class, names = "WORKBENCH", mode = EnumSource.Mode.EXCLUDE)
    void nonWorkbenchKindsAreRejected(BongEntityModelKind kind) {
        assertFalse(WorkbenchInteractIntentHandler.isWorkbenchKind(kind));
    }

    @Test
    void candidateReturnsEmptyWhenClientIsNull() {
        Optional<InteractCandidate> result = new WorkbenchInteractIntentHandler().candidate(null);
        assertFalse(result.isPresent(), "candidate(null) must return Optional.empty()");
    }

    @Test
    void candidateEntityIdParsesValidLabel() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer,
            10,
            1.0,
            "workbench:42"
        );
        assertEquals(42, WorkbenchInteractIntentHandler.candidateEntityId(candidate));
    }

    @Test
    void candidateEntityIdRejectsWrongPrefix() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer,
            10,
            1.0,
            "supply_coffin:42"
        );
        assertEquals(-1, WorkbenchInteractIntentHandler.candidateEntityId(candidate));
    }

    @Test
    void candidateEntityIdRejectsNullCandidate() {
        assertEquals(-1, WorkbenchInteractIntentHandler.candidateEntityId(null));
    }

    @Test
    void candidateEntityIdRejectsNonNumericSuffix() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer,
            10,
            1.0,
            "workbench:not_a_number"
        );
        assertEquals(-1, WorkbenchInteractIntentHandler.candidateEntityId(candidate));
    }

    // ============= isWithinInteractRange =============
    //
    // Pin tests mirroring server::craft::workbench::is_within_workbench_range
    // (server/src/craft/workbench.rs) boundary cases 1:1, to lock the Chebyshev
    // shape client-side and prevent plan-bughunt-entity-interact-range-desync-v1
    // from regressing (client euclidean sphere vs server Chebyshev cube drift).

    @Test
    void withinRangeAtOrigin() {
        assertTrue(WorkbenchInteractIntentHandler.isWithinInteractRange(
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        ), "expected in range because player stands exactly on the workbench block "
            + "(Chebyshev distance = 0.0 <= 3.0 boundary), actual was out-of-range");
    }

    @Test
    void withinRangeAtBoundaryOnSingleAxis() {
        // Exactly 3 blocks away on x axis must still be a candidate.
        assertTrue(WorkbenchInteractIntentHandler.isWithinInteractRange(
            3.0, 0.0, 0.0, 0.0, 0.0, 0.0
        ), "expected in range because Chebyshev distance = 3.0 is exactly at the inclusive "
            + "3.0 boundary (server accepts <= 3.0), actual was out-of-range");
    }

    @Test
    void withinRangeAtBoundaryOnAllAxes() {
        // Exactly 3 blocks away on all axes (Chebyshev = 3) must still be a candidate.
        assertTrue(WorkbenchInteractIntentHandler.isWithinInteractRange(
            3.0, 3.0, 3.0, 0.0, 0.0, 0.0
        ), "expected in range because Chebyshev distance = max(3,3,3) = 3.0 is exactly at "
            + "the inclusive boundary — a euclidean gate would wrongly reject this corner "
            + "(sqrt(27) ~ 5.2 > 3.0), actual was out-of-range");
    }

    @Test
    void outOfRangeJustBeyondBoundary() {
        assertFalse(
            WorkbenchInteractIntentHandler.isWithinInteractRange(
                3.1, 0.0, 0.0, 0.0, 0.0, 0.0
            ),
            "3.1 blocks on x axis must not produce a candidate — this is the exact "
                + "phantom-interact reproduction from plan-bughunt-entity-interact-range-desync-v1: "
                + "server rejects at this distance, client candidate must not disagree"
        );
    }

    @Test
    void withinRangeAtRealisticWorkbenchEntityOffset() {
        // Workbench entities are placed at block-center-x/z (+0.5) and block-floor-y,
        // per handle_workbench_place (server/src/craft/workbench.rs). A candidate must
        // floor the entity position back to the block coordinate before comparing —
        // not compare raw entity coordinates directly (that would shift the boundary
        // by 0.5 on x/z and desync from server again).
        assertTrue(WorkbenchInteractIntentHandler.isWithinInteractRange(
            3.0, 0.0, 0.5, 0.5, 0.0, 0.5
        ), "player 3.0 blocks from the block origin along x, with entity at +0.5 offset, "
            + "must still resolve to exactly 3.0 blocks after flooring — must be in range");

        assertFalse(WorkbenchInteractIntentHandler.isWithinInteractRange(
            3.1, 0.0, 0.5, 0.5, 0.0, 0.5
        ), "player 3.1 blocks from the block origin along x must be rejected even with "
            + "the entity's +0.5 visual offset");
    }

    @Test
    void withinRangeNegativeCoords() {
        assertTrue(WorkbenchInteractIntentHandler.isWithinInteractRange(
            -2.0, 5.0, -3.0, -5.0, 5.0, -3.0
        ), "expected in range because Chebyshev distance = max(3,0,0) = 3.0 in negative "
            + "coordinate space (mirrors server within_range_negative_coords), actual was "
            + "out-of-range — abs() handling of negative coords is likely broken");
    }

    @Test
    void outOfRangeFarAway() {
        assertFalse(WorkbenchInteractIntentHandler.isWithinInteractRange(
            100.0, 64.0, 200.0, 0.0, 64.0, 0.0
        ), "expected out of range because Chebyshev distance = max(100,0,200) = 200.0 "
            + "far exceeds the 3.0 boundary, actual was in-range");
    }

    @Test
    void withinRangeFractionalPlayerPos() {
        assertTrue(WorkbenchInteractIntentHandler.isWithinInteractRange(
            2.9, 0.5, 1.0, 0.0, 0.0, 0.0
        ), "expected in range because Chebyshev distance = max(2.9,0.5,1.0) = 2.9 < 3.0 "
            + "boundary with a fractional player position, actual was out-of-range");
    }
}
