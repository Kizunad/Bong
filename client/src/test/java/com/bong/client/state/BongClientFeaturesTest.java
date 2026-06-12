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
        // raw XML UI 通道必须保持 false 直到显式授权；agent-ui 走 AgentUiPayloadHandler 不读此 flag
        assertFalse(BongClientFeatures.ENABLE_DYNAMIC_XML_UI,
            "ENABLE_DYNAMIC_XML_UI 应保持 false（raw XML 通道未授权打开）；"
            + "若此断言失败，检查是否有代码将此 flag 翻 true（plan-agent-ui-data-v1 回归护栏）");
        assertFalse(BongClientFeatures.ENABLE_DEBUG_HEARTBEAT_CHAT);
    }
}
