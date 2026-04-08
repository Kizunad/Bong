package com.bong.client.state;

import org.junit.jupiter.api.Test;

import java.util.Locale;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NarrationStateTest {
    @Test
    void emptyFactoryReturnsNeutralState() {
        NarrationState state = NarrationState.empty();

        assertTrue(state.isEmpty());
        assertEquals(NarrationState.Scope.BROADCAST, state.scope());
        assertEquals(NarrationState.Style.NARRATION, state.style());
        assertTrue(state.target().isEmpty());
        assertEquals(0, state.toastDurationMillis());
        assertFalse(state.isToastEligible());
    }

    @Test
    void createClampsTextLengthAndFallsBackForUnknownWireValues() {
        String longText = " ".repeat(2) + "x".repeat(550) + " ";

        NarrationState state = NarrationState.create("mystery_scope", "  ignored-target  ", longText, "mystery_style");

        assertFalse(state.isEmpty());
        assertEquals(NarrationState.Scope.BROADCAST, state.scope());
        assertEquals(NarrationState.Style.NARRATION, state.style());
        assertTrue(state.target().isEmpty(), "broadcast narrations should drop target state");
        assertEquals(500, state.text().length());
        assertEquals(0, state.toastDurationMillis());
    }

    @Test
    void blankTextTransitionsToEmptyState() {
        NarrationState state = NarrationState.create("zone", "qiyun_peak", "   ", "system_warning");

        assertTrue(state.isEmpty());
        assertEquals(0, state.toastDurationMillis());
    }

    @Test
    void createParsesUppercaseWireNamesWithLocaleInvariantNormalization() {
        Locale previousLocale = Locale.getDefault();
        Locale.setDefault(Locale.forLanguageTag("tr"));
        try {
            NarrationState state = NarrationState.create("PLAYER", "  uuid-1  ", "感知到灵气波动", "PERCEPTION");

            assertFalse(state.isEmpty());
            assertEquals(NarrationState.Scope.PLAYER, state.scope());
            assertEquals("uuid-1", state.target().orElseThrow());
            assertEquals(NarrationState.Style.PERCEPTION, state.style());
            assertEquals(0, state.toastDurationMillis());
        } finally {
            Locale.setDefault(previousLocale);
        }
    }

    @Test
    void systemWarningAndEraDecreeExposeToastDurations() {
        NarrationState warning = NarrationState.create("player", "uuid-1", "Danger", "system_warning");
        NarrationState decree = NarrationState.create("broadcast", null, "Epoch shift", "era_decree");

        assertEquals(NarrationState.Scope.PLAYER, warning.scope());
        assertEquals("uuid-1", warning.target().orElseThrow());
        assertTrue(warning.isToastEligible());
        assertEquals(5_000, warning.toastDurationMillis());
        assertTrue(decree.isToastEligible());
        assertEquals(8_000, decree.toastDurationMillis());
    }
}
