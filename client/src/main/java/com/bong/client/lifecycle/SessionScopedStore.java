package com.bong.client.lifecycle;

@FunctionalInterface
public interface SessionScopedStore {
    void clearOnDisconnect();
}
