package com.bong.client.ui.intent;

import java.util.Objects;

/** 只表示本地传输结果；权威结果仍通过状态到达。 */
public record UiIntentResult(Kind kind, String reason, String requestId) {
    public UiIntentResult {
        Objects.requireNonNull(kind, "kind must not be null");
        reason = normalize(reason);
        requestId = normalize(requestId);
        if (kind != Kind.LOCAL_ACCEPTED && reason == null) {
            throw new IllegalArgumentException("rejected and error results require a reason");
        }
    }

    public static UiIntentResult accepted() {
        return new UiIntentResult(Kind.LOCAL_ACCEPTED, null, null);
    }

    public static UiIntentResult accepted(String requestId) {
        return new UiIntentResult(Kind.LOCAL_ACCEPTED, null, requestId);
    }

    public static UiIntentResult rejected(String reason) {
        return new UiIntentResult(Kind.LOCAL_REJECTED, reason, null);
    }

    public static UiIntentResult error(String reason) {
        return new UiIntentResult(Kind.LOCAL_ERROR, reason, null);
    }

    private static String normalize(String value) {
        if (value == null || value.isBlank()) {
            return null;
        }
        return value;
    }

    public enum Kind {
        LOCAL_ACCEPTED,
        LOCAL_REJECTED,
        LOCAL_ERROR
    }
}
