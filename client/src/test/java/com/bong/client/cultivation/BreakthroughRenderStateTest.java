package com.bong.client.cultivation;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F7：{@link BreakthroughRenderState} 边界锁定——isExpired 的闭/开区间 + null 校验。
 */
class BreakthroughRenderStateTest {

    @Test
    void constructor_rejectsNullPayload() {
        assertThrows(IllegalArgumentException.class, () -> new BreakthroughRenderState(null, 100L),
            "期望 payload=null 时构造抛 IllegalArgumentException");
    }

    @Test
    void isExpired_beforeExpiry_false() {
        BreakthroughRenderState state = new BreakthroughRenderState(payload(), 1_000L);
        assertFalse(state.isExpired(999L),
            "期望 now(999) < expiresAtMillis(1000) 时未过期，实际 isExpired=true");
    }

    @Test
    void isExpired_exactlyAtExpiry_true() {
        // 闭区间：now == expiresAtMillis 视为已过期（"到期"即失效，不需要再晚 1ms）
        BreakthroughRenderState state = new BreakthroughRenderState(payload(), 1_000L);
        assertTrue(state.isExpired(1_000L),
            "期望 now(1000) == expiresAtMillis(1000) 时已过期，实际 isExpired=false");
    }

    @Test
    void isExpired_afterExpiry_true() {
        BreakthroughRenderState state = new BreakthroughRenderState(payload(), 1_000L);
        assertTrue(state.isExpired(1_001L),
            "期望 now(1001) > expiresAtMillis(1000) 时已过期，实际 isExpired=false");
    }

    @Test
    void isExpired_zeroExpiry_alwaysExpired() {
        BreakthroughRenderState state = new BreakthroughRenderState(payload(), 0L);
        assertTrue(state.isExpired(0L));
        assertTrue(state.isExpired(1L));
    }

    private static BreakthroughCinematicPayload payload() {
        return new BreakthroughCinematicPayload(
            "actor",
            BreakthroughCinematicPayload.Phase.APEX,
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
