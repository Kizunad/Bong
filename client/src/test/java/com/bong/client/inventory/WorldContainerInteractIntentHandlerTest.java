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
