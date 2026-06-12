package com.bong.client.state;

import com.bong.client.network.ServerDataEnvelope;
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
        // plan-agent-ui-data-v1 P1: ENABLE_DYNAMIC_XML_UI 已设为 true；
        // 用显式 enabled=false 参数验证"关闭"分支行为仍正确（2-arg 变体已反映新默认值）。
        UiOpenState explicitlyDisabled = UiOpenState.dynamicXml("cultivation_panel", "<flow-layout/> ", false);
        UiOpenState unsafe = UiOpenState.dynamicXml("cultivation_panel", "<!DOCTYPE foo><flow-layout/>", true);

        assertTrue(explicitlyDisabled.isEmpty(),
            "enabled=false 时 dynamicXml 应返回 empty，实际非空（2026 P1）");
        assertTrue(unsafe.isEmpty());
    }

    @Test
    void explicitDynamicXmlEnablementStillGuardsSizeAndUnknownSafeNoOps() {
        String oversizeXml = buildDynamicXmlOfSize(ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1);
        UiOpenState oversize = UiOpenState.dynamicXml("cultivation_panel", oversizeXml, true);
        UiOpenState blank = UiOpenState.dynamicXml("   ", "<flow-layout/>", true);
        UiOpenState safe = UiOpenState.dynamicXml("cultivation_panel", "<flow-layout/>", true);

        assertTrue(oversize.isEmpty());
        assertEquals(ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1, oversizeXml.getBytes(java.nio.charset.StandardCharsets.UTF_8).length);
        assertTrue(blank.isEmpty());
        assertFalse(safe.isEmpty());
        assertTrue(safe.opensDynamicXml());
        assertEquals("cultivation_panel", safe.screenId());
        assertEquals("<flow-layout/>", safe.xmlLayout().orElseThrow());
    }

    private static String buildDynamicXmlOfSize(int targetSizeBytes) {
        String prefix = "<";
        String suffix = "/>";
        int bodyLength = targetSizeBytes - prefix.length() - suffix.length();
        if (bodyLength < 0) {
            throw new IllegalArgumentException("target size too small: " + targetSizeBytes);
        }

        return prefix + "x".repeat(bodyLength) + suffix;
    }
}
