package com.bong.client.state;

import com.bong.client.BongClientFeatures;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongClientFeaturesTest {
    @Test
    void dangerousFeaturesStayDisabledByDefault() {
        assertTrue(BongClientFeatures.ENABLE_TOASTS);
        assertTrue(BongClientFeatures.ENABLE_VISUAL_EFFECTS);
        assertTrue(BongClientFeatures.ENABLE_XML_TEMPLATE_MODE);
        // plan-agent-ui-data-v1 P1: ENABLE_DYNAMIC_XML_UI 已启用（天道动态 UI 管道实装）。
        assertTrue(BongClientFeatures.ENABLE_DYNAMIC_XML_UI,
            "plan-agent-ui-data-v1 P1 动态 XML UI 应已启用");
        assertFalse(BongClientFeatures.ENABLE_DEBUG_HEARTBEAT_CHAT);
    }
}
