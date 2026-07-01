package com.bong.client.cultivation;

import java.util.concurrent.atomic.AtomicReference;

/**
 * F7：突破演出渲染状态存取，模式仿 {@code com.bong.client.state.SeasonStateStore}。
 *
 * <p>{@link BreakthroughCinematicHandler} 收到 payload 时写入，
 * {@link BreakthroughBillboardWorldRenderer} 每帧读取 {@link #snapshot()} 决定是否画远景 billboard。
 * 单客户端主线程读写，不加锁，用 {@link AtomicReference} 只是防御式安全。
 */
public final class BreakthroughRenderStateStore {
    private static final AtomicReference<BreakthroughRenderState> STATE = new AtomicReference<>(null);

    private BreakthroughRenderStateStore() {
    }

    public static void replace(BreakthroughRenderState next) {
        STATE.set(next);
    }

    /** @return 最近一次写入的渲染状态；从未收到过 payload 时为 {@code null}。 */
    public static BreakthroughRenderState snapshot() {
        return STATE.get();
    }

    public static void resetForTests() {
        STATE.set(null);
    }
}
