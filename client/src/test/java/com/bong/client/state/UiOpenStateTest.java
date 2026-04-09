package com.bong.client.state;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class UiOpenStateTest {
    @Test
    void emptyFactoryRepresentsNoPendingUiOpen() {
        UiOpenState state = UiOpenState.empty();

        assertTrue(state.isEmpty());
        assertEquals(UiOpenState.Mode.NONE, state.mode());
        assertFalse(state.opensTemplate());
        assertFalse(state.opensDynamicXml());
        assertTrue(state.templateId().isEmpty());
        assertTrue(state.xmlLayout().isEmpty());
    }

    @Test
    void templateModeCanBeEnabledSeparately() {
        UiOpenState enabled = UiOpenState.template(" cultivation_panel ", " player_overview ", true);
        UiOpenState disabled = UiOpenState.template("cultivation_panel", "player_overview", false);

        assertFalse(enabled.isEmpty());
        assertTrue(enabled.opensTemplate());
        assertEquals("cultivation_panel", enabled.screenId());
        assertEquals("player_overview", enabled.templateId().orElseThrow());
        assertTrue(disabled.isEmpty());
    }

    @Test
    void rawXmlStaysDisabledByDefaultAndRejectsUnsafeContent() {
        UiOpenState defaultDisabled = UiOpenState.dynamicXml("cultivation_panel", "<flow-layout/> ");
        UiOpenState unsafe = UiOpenState.dynamicXml("cultivation_panel", "<!DOCTYPE foo><flow-layout/>", true);

        assertTrue(defaultDisabled.isEmpty(), "dynamic XML should be blocked by the default feature flag");
        assertTrue(unsafe.isEmpty());
    }

    @Test
    void explicitDynamicXmlEnablementStillGuardsSizeAndUnknownSafeNoOps() {
        String oversizeXml = "<" + "x".repeat(1_100) + "/>";
        UiOpenState oversize = UiOpenState.dynamicXml("cultivation_panel", oversizeXml, true);
        UiOpenState blank = UiOpenState.dynamicXml("   ", "<flow-layout/>", true);
        UiOpenState safe = UiOpenState.dynamicXml("cultivation_panel", "<flow-layout/>", true);

        assertTrue(oversize.isEmpty());
        assertTrue(blank.isEmpty());
        assertFalse(safe.isEmpty());
        assertTrue(safe.opensDynamicXml());
        assertEquals("cultivation_panel", safe.screenId());
        assertEquals("<flow-layout/>", safe.xmlLayout().orElseThrow());
    }
}
