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
}
