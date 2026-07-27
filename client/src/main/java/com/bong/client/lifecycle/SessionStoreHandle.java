package com.bong.client.lifecycle;

import java.util.Objects;

public record SessionStoreHandle(String fqcn, SessionScopedStore clearer) {
    public SessionStoreHandle {
        if (fqcn == null || fqcn.isBlank()) {
            throw new IllegalArgumentException("fqcn must not be blank");
        }
        Objects.requireNonNull(clearer, "clearer");
    }

    public void clearOnDisconnect() {
        clearer.clearOnDisconnect();
    }
}
