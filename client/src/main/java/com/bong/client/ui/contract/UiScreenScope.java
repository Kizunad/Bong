package com.bong.client.ui.contract;

/** 屏幕局部生命周期边界，不拥有会话 Store 的清理职责。 */
public interface UiScreenScope {
    void onOpen();

    void addCleanup(Runnable cleanup);

    void onTick(long nowMs);

    boolean runIfOpen(Runnable task);

    void close();

    boolean isClosed();
}
