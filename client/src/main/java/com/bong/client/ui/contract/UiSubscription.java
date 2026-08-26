package com.bong.client.ui.contract;

/** 一个状态订阅的幂等生命周期句柄。 */
public interface UiSubscription extends AutoCloseable {
    @Override
    void close();

    boolean isClosed();
}
