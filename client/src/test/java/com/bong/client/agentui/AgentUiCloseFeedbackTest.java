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
    void messageForReason_onlyNullOrEmptyReasonMeansSilentReplacement() {
        assertNull(AgentUiCloseFeedback.messageForReason(null));
        assertNull(AgentUiCloseFeedback.messageForReason(""));
        assertEquals("天道面板已失效，请重新尝试",
            AgentUiCloseFeedback.messageForReason("   "),
            "纯空白 reason 是非空未知值，应走可见兜底而非误判为 Replaced");
    }

    @Test
    void messageForReason_unknownReasonUsesVisibleForwardCompatibleFallback() {
        assertEquals("天道面板已失效，请重新尝试",
            AgentUiCloseFeedback.messageForReason("future_error_reason"));
    }

    @Test
    void showForReasonAt_publishesWarningToastWithExactLifetime() {
        long now = 10_000L;

        assertTrue(AgentUiCloseFeedback.showForReasonAt("session_expired", now),
            "session_expired 是错误 close，期望发布玩家可见 toast，实际返回 false");

        BongToast toast = BongToast.current(now);
        assertFalse(toast.isEmpty(),
            "showForReasonAt 返回 true 后应存在 active toast，实际 toast 为空");
        assertEquals("这次天道面板已过期，请重新尝试", toast.text().getString());
        assertEquals(AgentUiCloseFeedback.WARNING_COLOR, toast.color());
        assertEquals(now + AgentUiCloseFeedback.DURATION_MILLIS, toast.expiresAtMillis());
    }

    @Test
    void silentReplacementDoesNotOverwriteExistingToast() {
        BongToast.show("既有提示", 0xFFFFFF, 1_000L, 5_000L);

        assertFalse(AgentUiCloseFeedback.showForReasonAt(null, 2_000L),
            "null reason 表示 Replaced，期望静默且不替换既有 toast，实际返回 true");

        assertEquals("既有提示", BongToast.current(2_000L).text().getString());
    }
}
