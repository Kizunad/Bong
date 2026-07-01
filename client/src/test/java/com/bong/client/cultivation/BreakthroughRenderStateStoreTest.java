package com.bong.client.cultivation;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;

/**
 * F7：{@link BreakthroughRenderStateStore} 存取契约，模式仿
 * {@code com.bong.client.state.SeasonStateStore} 的测试风格。
 */
class BreakthroughRenderStateStoreTest {

    @AfterEach
    void tearDown() {
        BreakthroughRenderStateStore.resetForTests();
    }

    @Test
    void snapshot_defaultsToNull_beforeAnyReplace() {
        BreakthroughRenderStateStore.resetForTests();
        assertNull(BreakthroughRenderStateStore.snapshot(),
            "期望从未写入过时 snapshot() 为 null（渲染器据此判断'无演出中'）");
    }

    @Test
    void replace_thenSnapshot_returnsSameInstance() {
        BreakthroughRenderState state = new BreakthroughRenderState(payload("actorA"), 5_000L);
        BreakthroughRenderStateStore.replace(state);
        assertSame(state, BreakthroughRenderStateStore.snapshot(),
            "期望 snapshot() 返回上次 replace() 写入的同一实例");
    }

    @Test
    void replace_overwritesPreviousState() {
        BreakthroughRenderStateStore.replace(new BreakthroughRenderState(payload("actorA"), 1_000L));
        BreakthroughRenderState second = new BreakthroughRenderState(payload("actorB"), 2_000L);
        BreakthroughRenderStateStore.replace(second);
        assertSame(second, BreakthroughRenderStateStore.snapshot(),
            "期望第二次 replace() 覆盖第一次写入");
        assertEquals("actorB", BreakthroughRenderStateStore.snapshot().payload().actorId());
    }

    @Test
    void resetForTests_clearsToNull() {
        BreakthroughRenderStateStore.replace(new BreakthroughRenderState(payload("actorA"), 1_000L));
        BreakthroughRenderStateStore.resetForTests();
        assertNull(BreakthroughRenderStateStore.snapshot(),
            "期望 resetForTests() 后 snapshot() 回到 null");
    }

    @Test
    void replace_withNull_clearsStore() {
        BreakthroughRenderStateStore.replace(new BreakthroughRenderState(payload("actorA"), 1_000L));
        BreakthroughRenderStateStore.replace(null);
        assertNull(BreakthroughRenderStateStore.snapshot(),
            "期望 replace(null) 允许显式清空 store");
    }

    private static BreakthroughCinematicPayload payload(String actorId) {
        return new BreakthroughCinematicPayload(
            actorId,
            BreakthroughCinematicPayload.Phase.PRELUDE,
            0,
            80,
            "Condense",
            "Solidify",
            BreakthroughCinematicPayload.Result.PENDING,
            false,
            10.0,
            64.0,
            10.0,
            1024.0,
            false,
            true,
            1.0,
            0.5,
            "adaptive",
            "fresh_spiral",
            100L
        );
    }
}
