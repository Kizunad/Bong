package com.bong.client.ui.intent;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

class UiIntentContractTest {
    @Test
    void localTransportKindsKeepServerOutcomeSeparate() {
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, UiIntentResult.accepted().kind());
        assertEquals("req-1", UiIntentResult.accepted("req-1").requestId());
        assertEquals("blocked", UiIntentResult.rejected("blocked").reason());
        assertEquals("transport", UiIntentResult.error("transport").reason());
        assertNull(UiIntentResult.accepted().reason());
    }

    @Test
    void rejectedAndErrorResultsRequireReasons() {
        assertThrows(IllegalArgumentException.class,
            () -> new UiIntentResult(UiIntentResult.Kind.LOCAL_REJECTED, " ", null));
        assertThrows(IllegalArgumentException.class,
            () -> new UiIntentResult(UiIntentResult.Kind.LOCAL_ERROR, null, null));
        assertThrows(NullPointerException.class,
            () -> new UiIntentResult(null, "reason", null));
    }
}
