package com.bong.client.agentui;

import com.bong.client.hud.BongToast;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AgentUiCloseFeedbackTest {
    @AfterEach
    void tearDown() {
        BongToast.resetForTests();
    }

    @Test
    void messageForReason_mapsAllSchemaReasons() {
        assertEquals("天道拒绝了这次操作",
            AgentUiCloseFeedback.messageForReason("invalid_button_id"));
        assertEquals("这次天道面板已过期，请重新尝试",
            AgentUiCloseFeedback.messageForReason("session_expired"));
    }

    @Test
    void messageForReason_emptyReasonMeansSilentReplacement() {
        assertNull(AgentUiCloseFeedback.messageForReason(null));
        assertNull(AgentUiCloseFeedback.messageForReason(""));
        assertNull(AgentUiCloseFeedback.messageForReason("   "));
    }

    @Test
    void messageForReason_unknownReasonUsesVisibleForwardCompatibleFallback() {
        assertEquals("天道面板已失效，请重新尝试",
            AgentUiCloseFeedback.messageForReason("future_error_reason"));
    }

    @Test
    void showForReasonAt_publishesWarningToastWithExactLifetime() {
        long now = 10_000L;

        assertTrue(AgentUiCloseFeedback.showForReasonAt("session_expired", now));

        BongToast toast = BongToast.current(now);
        assertFalse(toast.isEmpty());
        assertEquals("这次天道面板已过期，请重新尝试", toast.text().getString());
        assertEquals(AgentUiCloseFeedback.WARNING_COLOR, toast.color());
        assertEquals(now + AgentUiCloseFeedback.DURATION_MILLIS, toast.expiresAtMillis());
    }

    @Test
    void silentReplacementDoesNotOverwriteExistingToast() {
        BongToast.show("既有提示", 0xFFFFFF, 1_000L, 5_000L);

        assertFalse(AgentUiCloseFeedback.showForReasonAt(null, 2_000L));

        assertEquals("既有提示", BongToast.current(2_000L).text().getString());
    }
}
