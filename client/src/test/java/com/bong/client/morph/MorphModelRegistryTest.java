package com.bong.client.morph;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 PR-5b — {@link MorphModelRegistry} 查询表单测。
 *
 * <p>覆盖：① 已知 form_race_id（whale）命中 ② 未知 form_race_id 安全 miss（渲染 mixin
 * 据此放行走 vanilla 玩家模型，不能崩/不能误判） ③ null 安全。
 */
class MorphModelRegistryTest {

    @Test
    void whaleHasModel() {
        assertTrue(MorphModelRegistry.hasModel("whale"));
    }

    @Test
    void unknownFormRaceIdHasNoModel() {
        assertFalse(MorphModelRegistry.hasModel("some_future_race"));
    }

    @Test
    void nullIsSafeAndHasNoModel() {
        assertFalse(MorphModelRegistry.hasModel(null));
    }

    @Test
    void emptyStringHasNoModel() {
        assertFalse(MorphModelRegistry.hasModel(""));
    }
}
