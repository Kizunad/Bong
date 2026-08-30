package com.bong.client.npc;

import com.bong.client.input.BongKeybindRegistry;
import net.minecraft.client.option.KeyBinding;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.BooleanSupplier;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NpcInteractionLogControlsTest {
    @BeforeEach
    void resetBeforeTest() {
        KeyBinding.unpressAll();
        NpcInteractionLogControls.resetControlsForTests();
        NpcInteractionLogStore.resetForTests();
    }

    @AfterEach
    void resetAfterTest() {
        KeyBinding.unpressAll();
        NpcInteractionLogControls.resetControlsForTests();
        NpcInteractionLogStore.resetForTests();
    }

    @Test
    void installsRegistrarResultAndConsumesIt() {
        List<KeyBinding> definitions = new ArrayList<>();

        KeyBinding installed = NpcInteractionLogControls.installInteractionLogKey(new BongKeybindRegistry(definition -> {
            definitions.add(definition);
            return new KeyBinding(
                "test.registered.npc_interaction_log",
                definition.getDefaultKey().getCategory(),
                GLFW.GLFW_KEY_F11,
                definition.getCategory()
            );
        }, List.of(), java.util.Set.of()));

        assertEquals(1, definitions.size());
        KeyBinding definition = definitions.get(0);
        assertEquals("key.bong-client.npc_interaction_log", definition.getTranslationKey());
        assertEquals("category.bong-client.controls", definition.getCategory());
        assertEquals(GLFW.GLFW_KEY_UNKNOWN, definition.getDefaultKey().getCode());
        assertTrue(definition.isUnbound(), "NPC 交互日志应默认未绑定");

        KeyBinding.onKeyPressed(installed.getDefaultKey());
        assertEquals(1, NpcInteractionLogControls.consumeInstalledTogglePresses(true, false));
        assertTrue(NpcInteractionLogStore.visible(),
            "生产消费链必须读取 registrar 返回并安装的同一绑定");
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
