package com.bong.client.inventory;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class WorldContainerInteractIntentHandlerTest {
    @Test
    void storageCrateModelIdsAreRecognised() {
        assertStorageCrateModel("trade_crate", true);
        assertStorageCrateModel("herb_crate_placed", true);
        assertStorageCrateModel("dead_drop_box", false);
        assertStorageCrateModel("workbench", false);
        assertStorageCrateModel(null, false);
    }

    @Test
    void deadDropModelIdIsRecognised() {
        assertDeadDropModel("dead_drop_box", true);
        assertDeadDropModel("trade_crate", false);
        assertDeadDropModel("herb_crate_placed", false);
        assertDeadDropModel(null, false);
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

    private static void assertStorageCrateModel(String modelId, boolean expected) {
        boolean actual = StorageCrateInteractIntentHandler.isStorageCrateModelId(modelId);
        assertEquals(
            expected,
            actual,
            "expected " + expected + " because modelId='" + modelId
                + "' must match storage-crate mapping only, actual " + actual
        );
    }

    private static void assertDeadDropModel(String modelId, boolean expected) {
        boolean actual = DeadDropInteractIntentHandler.isDeadDropModelId(modelId);
        assertEquals(
            expected,
            actual,
            "expected " + expected + " because modelId='" + modelId
                + "' must match dead-drop mapping only, actual " + actual
        );
    }
}
