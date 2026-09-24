package com.bong.client.lifecycle;

import java.util.Objects;

public final class SessionStoreHandle {
    private final Class<?> storeType;
    private final SessionScopedStore clearer;

    private SessionStoreHandle(Class<?> storeType, SessionScopedStore clearer) {
        this.storeType = Objects.requireNonNull(storeType, "storeType");
        this.clearer = Objects.requireNonNull(clearer, "clearer");
    }

    public static SessionStoreHandle forStore(
        Class<?> storeType,
        SessionScopedStore clearer
    ) {
        return new SessionStoreHandle(storeType, clearer);
    }

    public Class<?> storeType() {
        return storeType;
    }

    public String fqcn() {
        return storeType.getName();
    }

    public void clearOnDisconnect() {
        clearer.clearOnDisconnect();
    }
}
