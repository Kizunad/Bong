package com.bong.client.npc;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.BooleanSupplier;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NpcInteractionLogControlsTest {
    @BeforeEach
    void resetBeforeTest() {
        NpcInteractionLogStore.resetForTests();
    }

    @AfterEach
    void resetAfterTest() {
        NpcInteractionLogStore.resetForTests();
    }

    @Test
    void noPressLeavesInteractionLogVisibilityUnchanged() {
        int consumed = NpcInteractionLogControls.consumeTogglePresses(
            true,
            false,
            () -> false
        );

        assertEquals(0, consumed);
        assertFalse(NpcInteractionLogStore.visible());
    }

    @Test
    void explicitlyBoundPressTogglesInteractionLogOpenThenClosed() {
        assertEquals(1, NpcInteractionLogControls.consumeTogglePresses(
            true, false, presses(1)));
        assertTrue(NpcInteractionLogStore.visible(), "第一次显式绑定按键应打开交互日志");

        assertEquals(1, NpcInteractionLogControls.consumeTogglePresses(
            true, false, presses(1)));
        assertFalse(NpcInteractionLogStore.visible(), "第二次显式绑定按键应关闭交互日志");
    }

    @Test
    void missingPlayerDoesNotConsumeQueuedPressOrToggle() {
        AtomicInteger polls = new AtomicInteger();

        int consumed = NpcInteractionLogControls.consumeTogglePresses(
            false,
            false,
            () -> {
                polls.incrementAndGet();
                return true;
            }
        );

        assertEquals(0, consumed);
        assertEquals(0, polls.get(), "玩家不存在时不应提前排掉 wasPressed 队列");
        assertFalse(NpcInteractionLogStore.visible());
    }

    @Test
    void openScreenDoesNotConsumeQueuedPressOrToggle() {
        AtomicInteger polls = new AtomicInteger();

        int consumed = NpcInteractionLogControls.consumeTogglePresses(
            true,
            true,
            () -> {
                polls.incrementAndGet();
                return true;
            }
        );

        assertEquals(0, consumed);
        assertEquals(0, polls.get(), "界面打开时不应提前排掉 wasPressed 队列");
        assertFalse(NpcInteractionLogStore.visible());
    }

    @Test
    void drainsEveryQueuedWasPressedEdgeWhenWorldInputIsAllowed() {
        int consumed = NpcInteractionLogControls.consumeTogglePresses(
            true,
            false,
            presses(2)
        );

        assertEquals(2, consumed, "允许世界输入时应排空单 tick 累计的全部按键边沿");
        assertFalse(NpcInteractionLogStore.visible(), "连续两次按下应完成开再关状态转换");
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
