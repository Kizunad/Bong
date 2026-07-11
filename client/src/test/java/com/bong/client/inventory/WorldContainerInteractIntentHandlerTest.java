package com.bong.client.inventory;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class WorldContainerInteractIntentHandlerTest {
    @Test
    void storageCrateKindsAreRecognised() {
        assertStorageCrateKind(BongEntityModelKind.TRADE_CRATE, true);
        assertStorageCrateKind(BongEntityModelKind.HERB_CRATE_PLACED, true);
        assertStorageCrateKind(BongEntityModelKind.DEAD_DROP_BOX, false);
        assertStorageCrateKind(BongEntityModelKind.WORKBENCH, false);
        assertStorageCrateKind(null, false);
    }

    @Test
    void deadDropKindIsRecognised() {
        assertDeadDropKind(BongEntityModelKind.DEAD_DROP_BOX, true);
        assertDeadDropKind(BongEntityModelKind.TRADE_CRATE, false);
        assertDeadDropKind(BongEntityModelKind.HERB_CRATE_PLACED, false);
        assertDeadDropKind(null, false);
    }

    @Test
    void candidatesReturnEmptyWhenClientIsNull() {
        Optional<InteractCandidate> storage =
            new StorageCrateInteractIntentHandler().candidate(null);
        Optional<InteractCandidate> deadDrop =
            new DeadDropInteractIntentHandler().candidate(null);

        assertFalse(storage.isPresent(), "storage crate candidate(null) must be empty");
        assertFalse(deadDrop.isPresent(), "dead drop candidate(null) must be empty");
    }

    @Test
    void storageCrateCandidateEntityIdParsesOnlyOwnPrefix() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer,
            10,
            1.0,
            "storage_crate:42"
        );
        assertEquals(42, StorageCrateInteractIntentHandler.candidateEntityId(candidate));
        assertEquals(-1, DeadDropInteractIntentHandler.candidateEntityId(candidate));
    }

    @Test
    void deadDropCandidateEntityIdParsesOnlyOwnPrefix() {
        InteractCandidate candidate = InteractCandidate.of(
            InteractIntent.OpenContainer,
            10,
            1.0,
            "dead_drop:42"
        );
        assertEquals(42, DeadDropInteractIntentHandler.candidateEntityId(candidate));
        assertEquals(-1, StorageCrateInteractIntentHandler.candidateEntityId(candidate));
    }

    @Test
    void candidateEntityIdRejectsNullAndNonNumericSuffix() {
        assertEquals(-1, StorageCrateInteractIntentHandler.candidateEntityId(null));
        InteractCandidate bad = InteractCandidate.of(
            InteractIntent.OpenContainer,
            10,
            1.0,
            "storage_crate:not_a_number"
        );
        assertEquals(-1, StorageCrateInteractIntentHandler.candidateEntityId(bad));
    }

    private static void assertStorageCrateKind(BongEntityModelKind kind, boolean expected) {
        boolean actual = StorageCrateInteractIntentHandler.isStorageCrateKind(kind);
        assertEquals(
            expected,
            actual,
            "expected " + expected + " because kind='" + kind
                + "' must match storage-crate mapping only, actual " + actual
        );
    }

    // ============= ContainerOpenIntentSupport.isWithinInteractRange =============
    //
    // Pin tests locking the shared world-container gate to the server-side
    // OPEN_RANGE_BLOCKS + OPEN_RANGE_TOLERANCE = 4.5 boundary
    // (server/src/world/container_open.rs), preventing regression to the
    // client=5.0/server=4.5 drift from plan-bughunt-entity-interact-range-desync-v1.

    @Test
    void isWithinInteractRangeAcceptsExactlyAtBoundary() {
        double boundaryDistSq = 4.5 * 4.5;
        assertTrue(
            ContainerOpenIntentSupport.isWithinInteractRange(boundaryDistSq),
            "distSq exactly at 4.5^2 (server's OPEN_RANGE_BLOCKS + OPEN_RANGE_TOLERANCE) must be a candidate"
        );
    }

    @Test
    void isWithinInteractRangeRejectsJustBeyondBoundary() {
        double justBeyondDistSq = 4.51 * 4.51;
        assertFalse(
            ContainerOpenIntentSupport.isWithinInteractRange(justBeyondDistSq),
            "distSq just beyond 4.5 blocks must not be a candidate — server rejects here, "
                + "so this is exactly the phantom-interact window closed by "
                + "plan-bughunt-entity-interact-range-desync-v1 (old client gate was 5.0)"
        );
    }

    @Test
    void isWithinInteractRangeRejectsOldFivePointZeroWindow() {
        // The old client constant was 5.0 blocks. A distance strictly between the
        // server boundary (4.5) and the old client boundary (5.0) is exactly the
        // phantom-interact band this fix must close.
        double phantomWindowDistSq = 4.8 * 4.8;
        assertFalse(
            ContainerOpenIntentSupport.isWithinInteractRange(phantomWindowDistSq),
            "4.8 blocks is inside the old (incorrect) 5.0 client radius but outside the "
                + "true 4.5 server radius — must be rejected now"
        );
    }

    @Test
    void isWithinInteractRangeAcceptsZeroDistance() {
        assertTrue(ContainerOpenIntentSupport.isWithinInteractRange(0.0));
    }

    private static void assertDeadDropKind(BongEntityModelKind kind, boolean expected) {
        boolean actual = DeadDropInteractIntentHandler.isDeadDropKind(kind);
        assertEquals(
            expected,
            actual,
            "expected " + expected + " because kind='" + kind
                + "' must match dead-drop mapping only, actual " + actual
        );
    }
}
