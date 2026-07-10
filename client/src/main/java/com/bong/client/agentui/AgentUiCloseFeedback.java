package com.bong.client.agentui;

import com.bong.client.hud.BongToast;
import org.jetbrains.annotations.Nullable;

/**
 * {@code agent_ui_close.reason} 的玩家可见反馈映射。
 *
 * <p>空 reason 表示 Replaced，必须保持静默；错误 reason 走现有 BongToast HUD 活链路，
 * 避免错误关闭与正常替换在玩家视角退化成同一种静默收屏。
 */
final class AgentUiCloseFeedback {
    static final int WARNING_COLOR = 0xFFAA55;
    static final long DURATION_MILLIS = 3_000L;

    private AgentUiCloseFeedback() {}

    static boolean showForReason(@Nullable String reason) {
        return showForReasonAt(reason, System.currentTimeMillis());
    }

    static boolean showForReasonAt(@Nullable String reason, long nowMillis) {
        String message = messageForReason(reason);
        if (message == null) {
            return false;
        }
        BongToast.show(message, WARNING_COLOR, nowMillis, DURATION_MILLIS);
        return true;
    }

    @Nullable
    static String messageForReason(@Nullable String reason) {
        if (reason == null || reason.isEmpty()) {
            return null;
        }
        return switch (reason.trim()) {
            case "invalid_button_id" -> "天道拒绝了这次操作";
            case "session_expired" -> "这次天道面板已过期，请重新尝试";
            default -> "天道面板已失效，请重新尝试";
        };
    }
}
