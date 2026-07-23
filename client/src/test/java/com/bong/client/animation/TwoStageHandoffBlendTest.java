package com.bong.client.animation;

import dev.kosmx.playerAnim.api.TransformType;
import dev.kosmx.playerAnim.api.layered.AnimationStack;
import dev.kosmx.playerAnim.core.data.AnimationFormat;
import dev.kosmx.playerAnim.core.data.KeyframeAnimation;
import dev.kosmx.playerAnim.core.util.Ease;
import dev.kosmx.playerAnim.core.util.Vec3f;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-skill-anim-fidelity-v1 P6 —— 两段式「相位承接契约」的**客户端桥接 pin**
 * （conventions §14.2 裁决方案①：「fade 混合即为契约」）。
 *
 * <p>裁决要害是一条**不看源码就猜不到**的库语义链，本类把它钉成可回归的行为断言：
 *
 * <ol>
 *   <li>{@link AnimationLayerManager#playOnStack} 在同 channel 换 animId 时，先对旧
 *       animId 调 {@link BongAnimationPlayer#stopOnStack}（fadeOut &gt; 0 → 层**不立即
 *       摘除**，进 PENDING_REMOVALS 继续留在 AnimationStack），再把新 animId 作为
 *       新层 {@code addAnimLayer} 进栈；</li>
 *   <li>{@code AnimationStack.addAnimLayer} 对**同优先级**的插入点是「第一个
 *       priority ≥ 新值的位置」，于是**后进的 release 层位于先进的蓄力层之下**；
 *       而 {@code AnimationStack.get3DTransform} 自低索引向高索引逐层求值、把下层
 *       结果当作上层的 {@code value0}——即**正在淡出的蓄力段位于 release 段之上，
 *       向它混合下去**；</li>
 *   <li>{@code stopOnStack} 走的是 {@code replaceAnimationWithFade(fade, null)}，
 *       {@code ModifierLayer} 会把**淡出前的动画实例**挂成
 *       {@code AbstractFadeModifier.beginAnimation}，而
 *       {@code AbstractFadeModifier.get3DTransform} 的混合源正是
 *       {@code beginAnimation.get3DTransform(...)}——也就是蓄力段**当前相位**的姿态，
 *       不是 tick 0、更不是 vanilla 默认姿态。</li>
 * </ol>
 *
 * <p>三条合起来 = 引导窗停在任意相位都由「蓄力段当前姿态 → release 首帧」的真实
 * 交叉淡入承接，相位无关性是**结构性保证**而非资产巧合。这正是 P6 选方案①而非
 * ②（release 首帧改中性起手）/ ③（按相位选 release 变体）的依据：库已经把事情
 * 做对了，需要的是把它写进正典 + 钉住，而不是加资产或加分支。
 *
 * <p>反过来说，若哪天有人把交接改成「先摘层再播」「release 走更高 priority 抢占」
 * 或「fadeOut=0 立即摘层」，混合源就会塌回 vanilla 默认姿态、交接瞬间穿模跳变——
 * 本类的 {@code neverDipsTowardVanillaPose} 分支就是为这条回归设的。
 *
 * <p>用合成动画（{@code bong_test} 命名空间）而非真实资产：本类锁的是**桥接机制**，
 * 与具体招式姿态无关；真实资产各相位的姿态距离预算由
 * {@code AnimCastTicksAlignmentTest#twoStageHandoffHoldsAcrossEveryReachableLoopPhase}
 * 覆盖，两者互补不重叠。
 */
class TwoStageHandoffBlendTest {
    private static final Identifier LOOP_ID = new Identifier("bong_test", "handoff_loop");
    private static final Identifier RELEASE_ID = new Identifier("bong_test", "handoff_release");

    /** 生产两段式招统一使用的战斗档优先级（sword.infuse / yidao = 1200，anqi = 1500）。 */
    private static final int TWO_STAGE_PRIORITY = 1200;
    /** 蓄力段循环周期。 */
    private static final int LOOP_PERIOD = 20;
    /** 蓄力段 rightArm.pitch：全程停在 [1.5, 2.0]，**刻意远离 vanilla 0**。 */
    private static final float LOOP_PITCH_BASE = 1.5f;
    private static final float LOOP_PITCH_PEAK = 2.0f;
    /** release 段首帧 pitch：同样远离 0，与蓄力段构成一条不含 0 的区间 [0.8, 2.0]。 */
    private static final float RELEASE_PITCH_FIRST = 0.8f;

    private static final float EPS = 1e-3f;

    private UUID playerId;

    @BeforeEach
    void setUp() {
        BongAnimationPlayer.resetForTest();
        AnimationLayerManager.resetForTest();
        playerId = UUID.randomUUID();
        BongAnimationRegistry.register(LOOP_ID, buildLoop());
        BongAnimationRegistry.register(RELEASE_ID, buildRelease());
    }

    @AfterEach
    void tearDown() {
        BongAnimationPlayer.resetForTest();
        AnimationLayerManager.resetForTest();
    }

    /** 循环蓄力段：pitch 1.5 → 2.0 → 1.5，isLooped，周期 20t。 */
    private static KeyframeAnimation buildLoop() {
        KeyframeAnimation.AnimationBuilder b =
            new KeyframeAnimation.AnimationBuilder(AnimationFormat.UNKNOWN);
        b.endTick = LOOP_PERIOD;
        b.stopTick = LOOP_PERIOD + 2;
        b.isLooped = true;
        b.returnTick = 0;
        b.getPart("rightArm").pitch.addKeyFrame(0, LOOP_PITCH_BASE, Ease.INOUTSINE);
        b.getPart("rightArm").pitch.addKeyFrame(LOOP_PERIOD / 2, LOOP_PITCH_PEAK, Ease.INOUTSINE);
        b.getPart("rightArm").pitch.addKeyFrame(LOOP_PERIOD, LOOP_PITCH_BASE, Ease.INOUTSINE);
        return b.build();
    }

    /** release 段：首帧 0.8，非循环。 */
    private static KeyframeAnimation buildRelease() {
        KeyframeAnimation.AnimationBuilder b =
            new KeyframeAnimation.AnimationBuilder(AnimationFormat.UNKNOWN);
        b.endTick = 10;
        b.stopTick = 12;
        b.isLooped = false;
        b.returnTick = 0;
        b.getPart("rightArm").pitch.addKeyFrame(0, RELEASE_PITCH_FIRST, Ease.INOUTSINE);
        b.getPart("rightArm").pitch.addKeyFrame(10, RELEASE_PITCH_FIRST, Ease.INOUTSINE);
        return b.build();
    }

    /** 当前合成姿态的 rightArm.pitch（value0 = vanilla 中立 0，与生产求值起点一致）。 */
    private static float composedPitch(AnimationStack stack) {
        Vec3f out = stack.get3DTransform("rightArm", TransformType.ROTATION, 0f, Vec3f.ZERO);
        return out.getX();
    }

    private void playLoop(AnimationStack stack) {
        assertTrue(
            AnimationLayerManager.playOnStack(
                stack, playerId, AnimationLayerManager.Channel.UPPER_BODY,
                LOOP_ID, TWO_STAGE_PRIORITY, 0, BongAnimationPlayer.DEFAULT_FADE_OUT_TICKS),
            "蓄力段应成功起播");
    }

    /** 生产交接：同 channel 换 animId（内部先 StopAnim 旧层再新起 release 层）。 */
    private void handoffToRelease(AnimationStack stack, int fadeInTicks, int fadeOutTicks) {
        assertTrue(
            AnimationLayerManager.playOnStack(
                stack, playerId, AnimationLayerManager.Channel.UPPER_BODY,
                RELEASE_ID, TWO_STAGE_PRIORITY, fadeInTicks, fadeOutTicks),
            "release 段应成功接力");
    }

    /**
     * 核心断言：**在任意相位**交接，交接瞬间的合成姿态都等于蓄力段当时的姿态
     * ——没有跳变到 release 首帧，也没有塌回 vanilla。这就是「相位无关」的可观察形态。
     */
    @Test
    void handoffInstantPreservesLoopPoseAtEveryPhase() {
        for (int phase : List.of(0, LOOP_PERIOD / 4, LOOP_PERIOD / 2, LOOP_PERIOD - 1)) {
            BongAnimationPlayer.resetForTest();
            AnimationLayerManager.resetForTest();
            playerId = UUID.randomUUID();

            AnimationStack stack = new AnimationStack();
            playLoop(stack);
            for (int i = 0; i < phase; i++) {
                stack.tick();
            }
            float loopPose = composedPitch(stack);
            assertTrue(loopPose >= LOOP_PITCH_BASE - EPS && loopPose <= LOOP_PITCH_PEAK + EPS,
                "相位 " + phase + " 的蓄力段姿态 " + loopPose + " 应落在 ["
                    + LOOP_PITCH_BASE + ", " + LOOP_PITCH_PEAK + "]（测试自检）");

            handoffToRelease(stack, 1, 3);

            float atHandoff = composedPitch(stack);
            assertEquals(loopPose, atHandoff, EPS,
                "相位 " + phase + " 交接瞬间合成姿态应仍等于蓄力段当时姿态 " + loopPose
                    + "，实际 " + atHandoff + "——若等于 release 首帧 " + RELEASE_PITCH_FIRST
                    + " 说明发生硬跳变；若趋近 0 说明混合源塌回 vanilla（conventions §14.2）");
        }
    }

    /**
     * 交接全程合成姿态都夹在「蓄力段当时姿态」与「release 首帧」之间，**不穿过 vanilla**。
     * 两个端点都远离 0，所以一旦实现退化成「先摘层再播」或「两层各自与 vanilla 混合」，
     * 中途必然掉出区间被本用例抓住。
     */
    @Test
    void neverDipsTowardVanillaPoseDuringHandoff() {
        AnimationStack stack = new AnimationStack();
        playLoop(stack);
        for (int i = 0; i < LOOP_PERIOD / 2; i++) {
            stack.tick();
        }
        float loopPose = composedPitch(stack);
        float low = Math.min(loopPose, RELEASE_PITCH_FIRST) - EPS;
        float high = Math.max(loopPose, RELEASE_PITCH_FIRST) + EPS;

        handoffToRelease(stack, 1, 3);

        List<Float> trace = new ArrayList<>();
        for (int i = 0; i <= 6; i++) {
            float pose = composedPitch(stack);
            trace.add(pose);
            assertTrue(pose >= low && pose <= high,
                "交接第 " + i + " tick 合成姿态 " + pose + " 掉出 [" + low + ", " + high
                    + "]（蓄力段姿态 " + loopPose + " ↔ release 首帧 " + RELEASE_PITCH_FIRST
                    + "）——说明交接经由 vanilla 默认姿态中转，"
                    + "相位承接契约破裂。完整轨迹：" + trace);
            stack.tick();
            BongAnimationPlayer.tickPendingRemovalsForTest();
        }
    }

    /** 淡出走完后合成姿态收敛到 release 段，蓄力段层被回收（不泄漏）。 */
    @Test
    void handoffConvergesToReleasePoseAndReclaimsLoopLayer() {
        AnimationStack stack = new AnimationStack();
        playLoop(stack);
        for (int i = 0; i < LOOP_PERIOD / 2; i++) {
            stack.tick();
        }
        handoffToRelease(stack, 1, 3);

        // 淡出 3t + 摘层安全余量 1t = 4t 走完即可；**不能多跑到 release 自身
        // stopTick(12) 之后**——那时 release 也已播完失活，合成姿态回落 vanilla 属正常。
        for (int i = 0; i < 5; i++) {
            stack.tick();
            BongAnimationPlayer.tickPendingRemovalsForTest();
        }

        assertEquals(RELEASE_PITCH_FIRST, composedPitch(stack), EPS,
            "淡出走完、release 仍在播时，合成姿态应收敛到 release 段");
        assertEquals(0, BongAnimationPlayer.pendingRemovalsSize(),
            "蓄力段层应已回收——待摘队列残留 = 层泄漏");
        assertEquals(java.util.Set.of(RELEASE_ID),
            BongAnimationPlayer.activeAnimations(playerId),
            "交接后只应剩 release 段在播");
    }

    /**
     * 退化对照：{@code fadeOut = 0} 时旧层被**立即摘除**，混合源随之消失——交接瞬间
     * 合成姿态既不是蓄力段姿态、**也不是 release 首帧**，而是直接塌回 vanilla 中立
     * （release 自身的 fade-in 在 tick 0 进度为 0，混合源只剩下层的 {@code value0}）。
     *
     * <p>这比「硬跳到 release」更糟：玩家会看到手臂先弹回下垂再抬起。本用例把
     * 「{@code fadeOut > 0} 是相位承接契约的**必要条件**」钉死——淡出参数不是美观
     * 调节，是承接本身的载体（conventions §14.2）。
     */
    @Test
    void zeroFadeOutBreaksContinuityAndIsThereforeForbiddenForTwoStage() {
        AnimationStack stack = new AnimationStack();
        playLoop(stack);
        for (int i = 0; i < LOOP_PERIOD / 2; i++) {
            stack.tick();
        }
        float loopPose = composedPitch(stack);

        handoffToRelease(stack, 1, 0);

        float atHandoff = composedPitch(stack);
        assertEquals(0.0f, atHandoff, EPS,
            "fadeOut=0 时混合源被立即摘除，交接瞬间应塌回 vanilla 中立 0（记录退化形态本身）；"
                + "实际 " + atHandoff);
        assertTrue(Math.abs(atHandoff - loopPose) > 0.5f,
            "退化形态与蓄力段姿态应有肉眼可见落差——否则本对照用例失去意义："
                + "loopPose=" + loopPose + " atHandoff=" + atHandoff);
        assertTrue(Math.abs(atHandoff - RELEASE_PITCH_FIRST) > 0.5f,
            "退化形态同样偏离 release 首帧——证实塌回的是 vanilla 而非 release，"
                + "atHandoff=" + atHandoff + " releaseFirst=" + RELEASE_PITCH_FIRST);
    }
}
