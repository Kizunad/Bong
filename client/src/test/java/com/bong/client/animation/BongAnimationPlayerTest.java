package com.bong.client.animation;

import dev.kosmx.playerAnim.api.layered.AnimationStack;
import dev.kosmx.playerAnim.api.layered.KeyframeAnimationPlayer;
import dev.kosmx.playerAnim.api.layered.ModifierLayer;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 回归测试 stop 后的层清理——之前 stop() 只清本地 map 而不从 AnimationStack 摘层，
 * 反复 play→stop 会在 stack 里堆惰性 ModifierLayer，引起：
 * <ol>
 *   <li>内存泄漏——层被 AnimationStack.layers 持有</li>
 *   <li>性能退化——每 tick AnimationStack 会遍历全部 layers 调 isActive，
 *       即使全部 inactive 也是线性开销</li>
 * </ol>
 *
 * <p>修复是：stop() 把层推进 {@link BongAnimationPlayer#pendingRemovalsSize() pending 队列}，
 * {@code ClientTickEvents.END_CLIENT_TICK} 扣到 0 时调 {@link AnimationStack#removeLayer}。
 *
 * <p>为什么不直接驱动 play/stop 的全流程：stop 依赖 {@code AbstractClientPlayerEntity}
 * 和 {@code MinecraftClient.getInstance()}，单测里没这些运行时设施。转而验证核心逻辑：
 * 只要 pending 队列 + tick 行为正确，stop 就只是把（stack, layer, ticks）塞进队列的
 * 一个薄包装——这是本次修复的实质。
 */
public class BongAnimationPlayerTest {

    @BeforeEach
    void resetState() {
        BongAnimationPlayer.resetForTest();
    }

    @AfterEach
    void tearDown() {
        BongAnimationPlayer.resetForTest();
    }

    @Test
    void tickingDownRemovesLayerFromStackAtZero() {
        AnimationStack stack = new AnimationStack();
        ModifierLayer<KeyframeAnimationPlayer> layer = new ModifierLayer<>();
        stack.addAnimLayer(1000, layer);
        // 先塞一条要等 3 tick 的 removal
        BongAnimationPlayer.schedulePendingRemovalForTest(stack, layer, 3);
        assertEquals(1, BongAnimationPlayer.pendingRemovalsSize());

        // tick 1
        BongAnimationPlayer.tickPendingRemovalsForTest();
        assertEquals(1, BongAnimationPlayer.pendingRemovalsSize(), "还剩 2 tick，不该移除");
        assertTrue(stackContainsLayer(stack, layer), "还没到期，层还在 stack 里");

        // tick 2
        BongAnimationPlayer.tickPendingRemovalsForTest();
        assertEquals(1, BongAnimationPlayer.pendingRemovalsSize(), "还剩 1 tick，不该移除");

        // tick 3 → 移除
        BongAnimationPlayer.tickPendingRemovalsForTest();
        assertEquals(0, BongAnimationPlayer.pendingRemovalsSize(), "到 0 后出队");
        assertFalse(stackContainsLayer(stack, layer), "到 0 后从 stack 摘除");
    }

    @Test
    void immediateRemovalTicksUsesSingleTickCycle() {
        AnimationStack stack = new AnimationStack();
        ModifierLayer<KeyframeAnimationPlayer> layer = new ModifierLayer<>();
        stack.addAnimLayer(500, layer);
        // 1 tick 倒计时：一次 tick 就应该清干净
        BongAnimationPlayer.schedulePendingRemovalForTest(stack, layer, 1);

        BongAnimationPlayer.tickPendingRemovalsForTest();

        assertEquals(0, BongAnimationPlayer.pendingRemovalsSize());
        assertFalse(stackContainsLayer(stack, layer));
    }

    @Test
    void removesOnlyTargetLayerNotOthers() {
        AnimationStack stack = new AnimationStack();
        ModifierLayer<KeyframeAnimationPlayer> target = new ModifierLayer<>();
        ModifierLayer<KeyframeAnimationPlayer> innocent = new ModifierLayer<>();
        stack.addAnimLayer(500, innocent);
        stack.addAnimLayer(1000, target);

        BongAnimationPlayer.schedulePendingRemovalForTest(stack, target, 1);
        BongAnimationPlayer.tickPendingRemovalsForTest();

        assertFalse(stackContainsLayer(stack, target), "target 被摘");
        assertTrue(stackContainsLayer(stack, innocent), "其它层不受影响（引用等价移除）");
    }

    @Test
    void multiplePendingEntriesAllAdvanceEachTick() {
        AnimationStack stack = new AnimationStack();
        ModifierLayer<KeyframeAnimationPlayer> first = new ModifierLayer<>();
        ModifierLayer<KeyframeAnimationPlayer> second = new ModifierLayer<>();
        stack.addAnimLayer(500, first);
        stack.addAnimLayer(1000, second);

        BongAnimationPlayer.schedulePendingRemovalForTest(stack, first, 1);
        BongAnimationPlayer.schedulePendingRemovalForTest(stack, second, 2);

        BongAnimationPlayer.tickPendingRemovalsForTest();
        assertFalse(stackContainsLayer(stack, first), "first 在 1 tick 后移除");
        assertTrue(stackContainsLayer(stack, second), "second 还需再等 1 tick");
        assertEquals(1, BongAnimationPlayer.pendingRemovalsSize());

        BongAnimationPlayer.tickPendingRemovalsForTest();
        assertFalse(stackContainsLayer(stack, second), "second 在第 2 tick 后移除");
        assertEquals(0, BongAnimationPlayer.pendingRemovalsSize());
    }

    @Test
    void tickOnEmptyQueueIsNoOp() {
        assertEquals(0, BongAnimationPlayer.pendingRemovalsSize());
        BongAnimationPlayer.tickPendingRemovalsForTest();
        assertEquals(0, BongAnimationPlayer.pendingRemovalsSize());
    }

    /** AnimationStack 没有公共的 "contains" API，只能靠 removeLayer 的返回值探测——
     *  但 removeLayer 有副作用。改成：再次 addAnimLayer + removeLayer（两个都是幂等检测）。
     *  简化为：用 reflect 偷看 private layers 字段长度变化。 */
    private static boolean stackContainsLayer(AnimationStack stack, Object layer) {
        // AnimationStack.removeLayer 使用 reference equality，临时移除 → 若返回 true 说明原本在
        // 之后再 add 回去保持 stack 状态不变
        boolean wasPresent = stack.removeLayer((dev.kosmx.playerAnim.api.layered.IAnimation) layer);
        if (wasPresent) {
            // 恢复——priority 在这里不重要，因为验证都看引用存在性
            stack.addAnimLayer(0, (dev.kosmx.playerAnim.api.layered.IAnimation) layer);
        }
        return wasPresent;
    }
}
