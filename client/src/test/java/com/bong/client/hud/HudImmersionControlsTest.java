package com.bong.client.hud;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.concurrent.atomic.AtomicLong;
import java.util.function.BooleanSupplier;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class HudImmersionControlsTest {
    @BeforeEach
    void resetBeforeTest() {
        HudImmersionMode.resetForTests();
    }

    @AfterEach
    void resetAfterTest() {
        HudImmersionMode.resetForTests();
    }

    @Test
    void noPressLeavesManualImmersionUnchangedWithoutReadingClock() {
        int consumed = HudImmersionControls.consumeTogglePresses(
            () -> false,
            () -> {
                throw new AssertionError("没有按键事件时不应读取时钟");
            }
        );

        assertEquals(0, consumed);
        assertFalse(HudImmersionMode.manualImmersive());
    }

    @Test
    void explicitlyBoundPressTogglesManualImmersionOnThenOff() {
        assertEquals(1, HudImmersionControls.consumeTogglePresses(presses(1), () -> 1_000L));
        assertTrue(HudImmersionMode.manualImmersive(), "第一次显式绑定按键应开启沉浸模式");
        assertEquals(1.0, HudImmersionMode.immersiveAlpha(true, false, 1_000L), 0.0001,
            "按键消费必须使用注入时钟记录淡出起点");

        assertEquals(1, HudImmersionControls.consumeTogglePresses(presses(1), () -> 2_000L));
        assertFalse(HudImmersionMode.manualImmersive(), "第二次显式绑定按键应关闭沉浸模式");
        assertEquals(0.0, HudImmersionMode.immersiveAlpha(false, false, 2_000L), 0.0001,
            "关闭时也必须使用本次按键的注入时间启动淡入");
    }

    @Test
    void drainsEveryQueuedWasPressedEdgeInOneTick() {
        AtomicLong now = new AtomicLong(3_000L);

        int consumed = HudImmersionControls.consumeTogglePresses(
            presses(2),
            now::getAndIncrement
        );

        assertEquals(2, consumed, "单 tick 内累计的 wasPressed 边沿必须全部排空");
        assertFalse(HudImmersionMode.manualImmersive(), "连续两次按下应完成开再关状态转换");
    }

    private static BooleanSupplier presses(int count) {
        int[] remaining = {count};
        return () -> {
            if (remaining[0] <= 0) {
                return false;
            }
            remaining[0]--;
            return true;
        };
    }
}
