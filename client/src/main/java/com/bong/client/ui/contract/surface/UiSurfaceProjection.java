package com.bong.client.ui.contract.surface;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** 不携带渲染器、DOM、XML 或像素坐标的不可变语义界面。 */
public record UiSurfaceProjection(
    String surfaceId,
    String templateId,
    String sessionId,
    long revision,
    long expiresAtMs,
    String closeReason,
    Map<String, String> viewData,
    Map<String, String> collectionIdentity,
    Map<String, UiActionSpec> allowedActions
) {
    public static final long NO_EXPIRY = -1L;

    public UiSurfaceProjection {
        surfaceId = requireId(surfaceId, "surfaceId");
        templateId = requireId(templateId, "templateId");
        sessionId = requireId(sessionId, "sessionId");
        if (revision < 0L) {
            throw new IllegalArgumentException("revision must be non-negative");
        }
        if (expiresAtMs < NO_EXPIRY) {
            throw new IllegalArgumentException("expiresAtMs must be -1 or non-negative");
        }
        closeReason = normalize(closeReason);
        viewData = copyStringMap(viewData, "viewData");
        collectionIdentity = copyStringMap(collectionIdentity, "collectionIdentity");
        allowedActions = copyActions(allowedActions);
    }

    public boolean isClosed() {
        return closeReason != null;
    }

    public boolean isExpired(long nowMs) {
        return expiresAtMs != NO_EXPIRY && nowMs >= expiresAtMs;
    }

    public UiActionSpec action(String actionId) {
        return allowedActions.get(actionId);
    }

    private static Map<String, String> copyStringMap(Map<String, String> source, String name) {
        Objects.requireNonNull(source, name + " must not be null");
        Map<String, String> copy = new LinkedHashMap<>();
        source.forEach((key, value) -> copy.put(
            requireId(key, name + " key"),
            Objects.requireNonNull(value, name + " value must not be null")
        ));
        return Collections.unmodifiableMap(copy);
    }

    private static Map<String, UiActionSpec> copyActions(Map<String, UiActionSpec> source) {
        Objects.requireNonNull(source, "allowedActions must not be null");
        Map<String, UiActionSpec> copy = new LinkedHashMap<>();
        source.forEach((key, action) -> {
            String actionId = requireId(key, "allowedActions key");
            UiActionSpec checked = Objects.requireNonNull(action, "action must not be null");
            if (!actionId.equals(checked.actionId())) {
                throw new IllegalArgumentException("action map key must match actionId: " + actionId);
            }
            copy.put(actionId, checked);
        });
        return Collections.unmodifiableMap(copy);
    }

    private static String requireId(String value, String name) {
        Objects.requireNonNull(value, name + " must not be null");
        if (value.isBlank()) {
            throw new IllegalArgumentException(name + " must not be blank");
        }
        return value;
    }

    private static String normalize(String value) {
        return value == null || value.isBlank() ? null : value;
    }
}
