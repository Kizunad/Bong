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
        // 通过无 flag 参数的 2-arg 变体（读 BongClientFeatures.ENABLE_DYNAMIC_XML_UI）验证：
        // 默认 flag 态（false）下 dynamicXml raw-XML 通道应被拦截（返回 empty）。
        // 任何将 ENABLE_DYNAMIC_XML_UI 改为 true 的变动都会让这条用例立即红，保护安全副作用护栏。
        UiOpenState defaultFlagResult = UiOpenState.dynamicXml("cultivation_panel", "<flow-layout/>");
        UiOpenState unsafe = UiOpenState.dynamicXml("cultivation_panel", "<!DOCTYPE foo><flow-layout/>", true);

        assertTrue(defaultFlagResult.isEmpty(),
            "ENABLE_DYNAMIC_XML_UI=false（默认态）时 dynamicXml 应返回 empty，"
            + "raw XML 通道未授权打开；若此断言失败，检查 BongClientFeatures.ENABLE_DYNAMIC_XML_UI 是否被翻 true");
        assertTrue(unsafe.isEmpty(),
            "含 DOCTYPE 的 XML 应被拦截，unsafe content guard 失效");
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
