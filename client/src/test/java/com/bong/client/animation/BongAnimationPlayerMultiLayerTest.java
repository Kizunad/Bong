package com.bong.client.animation;

import dev.kosmx.playerAnim.api.layered.AnimationStack;
import dev.kosmx.playerAnim.api.layered.IAnimation;
import dev.kosmx.playerAnim.core.data.AnimationFormat;
import dev.kosmx.playerAnim.core.data.KeyframeAnimation;
import dev.kosmx.playerAnim.core.util.Pair;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-player-animation-v1 §3.3 多层 priority 叠加回归测试——"行走 + 挥剑同时"。
 *
 * <p>Bong 的 priority 档位（§3.3）：
 * <ul>
 *   <li>100-499 姿态（打坐、悬浮）</li>
 *   <li>500-999 移动（走路、轻功）</li>
 *   <li>1000-1999 战斗（挥剑、出掌）</li>
 *   <li>2000-2999 受击 / 倒地</li>
 *   <li>3000+ 剧情（突破、天劫）</li>
 * </ul>
 *
 * <p>验证要点：
 * <ol>
 *   <li>不同 animId + 不同 priority 同时播放互不干扰——各自一个独立
 *       {@code ModifierLayer}，都挂在玩家的 AnimationStack 上</li>
 *   <li>stop 其中一个不影响另一个</li>
 *   <li>AnimationStack 内部按 priority 升序排列（高 priority 会覆盖低
 *       priority 的 3D transform，这是 PlayerAnimator 的契约）</li>
 *   <li>相同 animId 再次 play 走 replaceAnimationWithFade，不应新增 AnimationStack
 *       条目——否则连续触发会让层数爆炸</li>
 * </ol>
 *
 * <p>测试通过 {@link BongAnimationPlayer#playOnStack} seam 绕过
 * {@code AbstractClientPlayerEntity} 和 {@code PlayerAnimationAccess} ——
 * 后者依赖 Mixin 注入的 IPlayer 接口，单测 classloader 里无法提供。
 */
public class BongAnimationPlayerMultiLayerTest {
    private static final Identifier WALK_ID = new Identifier("bong_test", "walk");
    private static final Identifier SWORD_ID = new Identifier("bong_test", "sword_swing");
    private static final Identifier MEDITATE_ID = new Identifier("bong_test", "meditate");

    /** §3.3 姿态档（打坐）。 */
    private static final int POSTURE_PRIORITY = 200;
    /** §3.3 移动档（走路）。 */
    private static final int MOVEMENT_PRIORITY = 750;
    /** §3.3 战斗档（挥剑）。 */
    private static final int COMBAT_PRIORITY = 1500;

    private UUID playerId;

    @BeforeEach
    void setUp() {
        BongAnimationPlayer.resetForTest();
        playerId = UUID.randomUUID();
        // Java fallback 注册最小动画——JSON 路径在测试 classloader 里走不通
        // (PlayerAnimationRegistry 依赖资源包系统)，这里只需要 get() 能返回非 null 的实例
        BongAnimationRegistry.register(WALK_ID, buildMinimalAnim());
        BongAnimationRegistry.register(SWORD_ID, buildMinimalAnim());
        BongAnimationRegistry.register(MEDITATE_ID, buildMinimalAnim());
    }

    @AfterEach
    void tearDown() {
        BongAnimationPlayer.resetForTest();
        // 注册表没有 unregister API，测试用的 id 留在 map 里——用 bong_test namespace
        // 避免污染 bong namespace 的补全
    }

    @Test
    void playMultipleAnimsOfDifferentPrioritiesCoexist() {
        AnimationStack stack = new AnimationStack();

        assertTrue(BongAnimationPlayer.playOnStack(stack, playerId, WALK_ID, MOVEMENT_PRIORITY, 0));
        assertTrue(BongAnimationPlayer.playOnStack(stack, playerId, SWORD_ID, COMBAT_PRIORITY, 0));

        Set<Identifier> active = BongAnimationPlayer.activeAnimations(playerId);
        assertEquals(2, active.size(), "两个 animId 都应在激活集合里");
        assertTrue(active.contains(WALK_ID));
        assertTrue(active.contains(SWORD_ID));

        assertEquals(2, layersIn(stack).size(), "AnimationStack 应有 2 层");
    }

    @Test
    void threeTierStackingPostureMovementCombat() {
        // 最能代表"多档同时"场景：打坐 + 走路 + 挥剑不可能物理上同时做，但技术契约必须支持
        // 任意档位组合（比如 撑劫姿态 200 + 御剑移动 300 + 挥剑 1500）
        AnimationStack stack = new AnimationStack();

        BongAnimationPlayer.playOnStack(stack, playerId, MEDITATE_ID, POSTURE_PRIORITY, 0);
        BongAnimationPlayer.playOnStack(stack, playerId, WALK_ID, MOVEMENT_PRIORITY, 0);
        BongAnimationPlayer.playOnStack(stack, playerId, SWORD_ID, COMBAT_PRIORITY, 0);

        assertEquals(3, BongAnimationPlayer.activeAnimations(playerId).size());
        List<Pair<Integer, IAnimation>> layers = layersIn(stack);
        assertEquals(3, layers.size());

        // PlayerAnimator 的 AnimationStack.addAnimLayer 按 priority 升序插入；低优先级在前
        // 确保 COMBAT 的 transform 最后 evaluate —— 这是高档覆盖低档的前提
        assertEquals(POSTURE_PRIORITY, (int) layers.get(0).getLeft());
        assertEquals(MOVEMENT_PRIORITY, (int) layers.get(1).getLeft());
        assertEquals(COMBAT_PRIORITY, (int) layers.get(2).getLeft());
    }

    @Test
    void stopOneAnimLeavesOthersUntouched() {
        AnimationStack stack = new AnimationStack();
        BongAnimationPlayer.playOnStack(stack, playerId, WALK_ID, MOVEMENT_PRIORITY, 0);
        BongAnimationPlayer.playOnStack(stack, playerId, SWORD_ID, COMBAT_PRIORITY, 0);

        // fadeOut=0 走立即摘层分支，状态更干净（不用等 tick）
        assertTrue(BongAnimationPlayer.stopOnStack(stack, playerId, SWORD_ID, 0));

        Set<Identifier> active = BongAnimationPlayer.activeAnimations(playerId);
        assertEquals(1, active.size(), "只剩 WALK");
        assertTrue(active.contains(WALK_ID));
        assertFalse(active.contains(SWORD_ID));
        assertEquals(1, layersIn(stack).size());
    }

    @Test
    void retriggeringSameAnimIdDoesNotAddLayer() {
        // 本次修复的核心不变式：连击同一个 animId 应走 replaceAnimationWithFade，
        // 不能在 AnimationStack 里堆 N 个同 animId 的层——否则连续打拳 30 次层数直接爆
        AnimationStack stack = new AnimationStack();
        assertTrue(BongAnimationPlayer.playOnStack(stack, playerId, SWORD_ID, COMBAT_PRIORITY, 0));
        int layerCountAfterFirst = layersIn(stack).size();
        assertEquals(1, layerCountAfterFirst);

        // 再 play 4 次，每次应只 replaceAnimationWithFade，不新增层
        for (int i = 0; i < 4; i++) {
            assertTrue(BongAnimationPlayer.playOnStack(stack, playerId, SWORD_ID, COMBAT_PRIORITY, 3));
        }

        assertEquals(1, layersIn(stack).size(), "重触发应复用层而不是新建");
        assertEquals(1, BongAnimationPlayer.activeAnimations(playerId).size());
    }

    @Test
    void differentAnimIdsMapToDistinctModifierLayers() {
        // 虽然不是用户能直接观察的行为，但这是 ACTIVE_LAYERS 存层的隐含契约——
        // 同 key 返回同引用，不同 key 返回不同引用
        AnimationStack stack = new AnimationStack();
        BongAnimationPlayer.playOnStack(stack, playerId, WALK_ID, MOVEMENT_PRIORITY, 0);
        BongAnimationPlayer.playOnStack(stack, playerId, SWORD_ID, COMBAT_PRIORITY, 0);

        List<Pair<Integer, IAnimation>> layers = layersIn(stack);
        assertNotSame(layers.get(0).getRight(), layers.get(1).getRight(),
            "两个不同 animId 必须对应不同 ModifierLayer");
    }

    @Test
    void stopNonExistentAnimReturnsFalseDoesNotRemoveOthers() {
        AnimationStack stack = new AnimationStack();
        BongAnimationPlayer.playOnStack(stack, playerId, WALK_ID, MOVEMENT_PRIORITY, 0);

        Identifier ghostId = new Identifier("bong_test", "not_registered_here");
        assertFalse(BongAnimationPlayer.stopOnStack(stack, playerId, ghostId, 0),
            "stop 不存在的 animId 返回 false");

        assertEquals(1, layersIn(stack).size(), "WALK 不受影响");
        assertTrue(BongAnimationPlayer.activeAnimations(playerId).contains(WALK_ID));
    }

    // --- 测试辅助 ---

    /** 构造一个最小但合法的 KeyframeAnimation——只需要 get() 能返回非 null 让
     *  playOnStack 进后续路径，不关心动画内容。 */
    private static KeyframeAnimation buildMinimalAnim() {
        KeyframeAnimation.AnimationBuilder b = new KeyframeAnimation.AnimationBuilder(AnimationFormat.UNKNOWN);
        b.endTick = 1;
        b.isLooped = false;
        return b.build();
    }

    /** 反射读 {@code AnimationStack.layers}（private ArrayList&lt;Pair&lt;Integer, IAnimation&gt;&gt;）。
     *  AnimationStack 未暴露 layers() getter，测试需要看"有哪些 priority / 哪几层"只能走反射。 */
    @SuppressWarnings("unchecked")
    private static List<Pair<Integer, IAnimation>> layersIn(AnimationStack stack) {
        try {
            Field f = AnimationStack.class.getDeclaredField("layers");
            f.setAccessible(true);
            Object raw = f.get(stack);
            assertNotNull(raw, "layers 字段应非 null");
            // ArrayList 直接转；拷贝一份避免测试误改底层
            return new ArrayList<>((List<Pair<Integer, IAnimation>>) raw);
        } catch (ReflectiveOperationException ex) {
            throw new AssertionError("反射 AnimationStack.layers 失败", ex);
        }
    }
}
