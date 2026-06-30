package com.bong.client.fauna;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * {@link FaunaActionAnimation} 一次性招式动画状态机的饱和测试。
 *
 * <p>锁住 controller 的决策契约（{@link FaunaActionAnimation#currentAnim()} 决定播招式 vs idle）
 * 与倒计时生命周期，无需 bootstrap GeckoLib 运行时。
 */
class FaunaActionAnimationTest {
    private static final String ANIM = "animation.bong.heiwushi.dark_barrage";

    @Test
    void freshStateHasNoAction() {
        FaunaActionAnimation action = new FaunaActionAnimation();
        assertFalse(action.hasAction(), "新建状态机不应有招式动画");
        assertNull(action.currentAnim(), "无招式时 currentAnim 应为 null（controller 走 idle）");
        assertEquals(0, action.remainingTicks());
    }

    @Test
    void triggerSetsAnimAndTicks() {
        FaunaActionAnimation action = new FaunaActionAnimation();
        action.trigger(ANIM, 15);
        assertTrue(action.hasAction(), "trigger 后应有招式动画");
        assertEquals(ANIM, action.currentAnim(), "currentAnim 应为触发的动画名（controller 据此播招式）");
        assertEquals(15, action.remainingTicks());
    }

    @Test
    void tickCountsDownAndClearsAtZero() {
        FaunaActionAnimation action = new FaunaActionAnimation();
        action.trigger(ANIM, 3);
        action.tick();
        assertEquals(2, action.remainingTicks());
        assertEquals(ANIM, action.currentAnim(), "倒计时未归零，招式动画应仍在播");
        action.tick();
        assertEquals(1, action.remainingTicks());
        assertTrue(action.hasAction());
        action.tick();
        // 第 3 次 tick 归零 → 清空，controller 回 idle。
        assertEquals(0, action.remainingTicks());
        assertFalse(action.hasAction(), "倒计时归零后应清空招式（controller 回 idle）");
        assertNull(action.currentAnim());
    }

    @Test
    void tickOnIdleStateIsNoop() {
        FaunaActionAnimation action = new FaunaActionAnimation();
        // 无招式时 tick 不应让 remainingTicks 变负 / 抛异常。
        action.tick();
        action.tick();
        assertEquals(0, action.remainingTicks());
        assertFalse(action.hasAction());
    }

    @Test
    void durationOneClearsAfterSingleTick() {
        // 边界：duration=1（下界）→ 一次 tick 即清空。
        FaunaActionAnimation action = new FaunaActionAnimation();
        action.trigger(ANIM, 1);
        assertTrue(action.hasAction());
        action.tick();
        assertFalse(action.hasAction(), "duration=1 应在单次 tick 后清空");
        assertEquals(0, action.remainingTicks());
    }

    @Test
    void retriggerReplacesCurrentAction() {
        FaunaActionAnimation action = new FaunaActionAnimation();
        action.trigger(ANIM, 15);
        action.tick();
        // 倒计时途中再次触发新招式 → 覆盖动画名 + 重置 ticks。
        action.trigger("animation.bong.heiwushi.dark_vortex", 21);
        assertEquals("animation.bong.heiwushi.dark_vortex", action.currentAnim());
        assertEquals(21, action.remainingTicks(), "re-trigger 应重置 ticks 到新时长");
    }

    @Test
    void triggerIgnoresNullBlankNameAndNonPositiveDuration() {
        FaunaActionAnimation action = new FaunaActionAnimation();
        // 防御：无效输入不应建立招式态（否则 controller 会播空动画 or 卡死）。
        action.trigger(null, 15);
        assertFalse(action.hasAction(), "null 动画名应被忽略");
        action.trigger("   ", 15);
        assertFalse(action.hasAction(), "空白动画名应被忽略");
        action.trigger(ANIM, 0);
        assertFalse(action.hasAction(), "duration=0 应被忽略");
        action.trigger(ANIM, -5);
        assertFalse(action.hasAction(), "duration<0 应被忽略");
    }

    @Test
    void invalidTriggerDoesNotClobberActiveAction() {
        FaunaActionAnimation action = new FaunaActionAnimation();
        action.trigger(ANIM, 10);
        // 正在播招式时收到无效触发 → 不应把当前招式清掉。
        action.trigger(null, 5);
        action.trigger(ANIM, 0);
        assertTrue(action.hasAction(), "无效触发不应清掉正在播的招式");
        assertEquals(ANIM, action.currentAnim());
        assertEquals(10, action.remainingTicks(), "无效触发不应改动当前 ticks");
    }
}
