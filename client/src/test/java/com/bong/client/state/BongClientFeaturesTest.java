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
        assertFalse(BongClientFeatures.ENABLE_DYNAMIC_XML_UI, "raw XML UI must stay disabled until explicitly enabled");
        assertFalse(BongClientFeatures.ENABLE_DEBUG_HEARTBEAT_CHAT);
    }
}
