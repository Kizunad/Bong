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
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 断线动画终结清理：旧 stack 层、channel ownership 与延迟 impact closure 都不得跨 session。 */
class AnimationDisconnectCleanupTest {
    private static final Identifier WALK = new Identifier("bong_test", "disconnect_walk");
    private static final Identifier FIST = new Identifier("bong_test", "disconnect_fist");

    private UUID playerId;

    @BeforeEach
    void setUp() {
        BongAnimationPlayer.resetForTest();
        AnimationLayerManager.resetForTest();
        BongPunchCombo.clearOnDisconnect();
        playerId = UUID.randomUUID();
        BongAnimationRegistry.register(WALK, minimalAnimation());
        BongAnimationRegistry.register(FIST, minimalAnimation());
    }

    @AfterEach
    void tearDown() {
        BongPunchCombo.clearOnDisconnect();
        AnimationLayerManager.resetForTest();
        BongAnimationPlayer.resetForTest();
    }

    @Test
    void disconnectPhysicallyRemovesActiveAndPendingLayersFromTheirOriginalStacks() {
        AnimationStack activeStack = new AnimationStack();
        AnimationStack pendingStack = new AnimationStack();

        assertTrue(BongAnimationPlayer.playOnStack(activeStack, playerId, WALK, 500, 0));
        assertTrue(BongAnimationPlayer.playOnStack(pendingStack, playerId, FIST, 1000, 0));
        assertTrue(BongAnimationPlayer.stopOnStack(pendingStack, playerId, FIST, 5));

        assertEquals(1, layerCount(activeStack), "前置：active 层必须留在它自己的旧 stack");
        assertEquals(1, layerCount(pendingStack), "前置：pending 层必须留在它自己的旧 stack");
        assertEquals(1, BongAnimationPlayer.pendingRemovalsSize(), "前置：淡出层必须进入 pending 队列");

        BongAnimationPlayer.clearOnDisconnect();

        assertEquals(0, layerCount(activeStack), "断线必须从 active layer 的原始 stack 物理摘除层");
        assertEquals(0, layerCount(pendingStack), "断线必须从 pending layer 的原始 stack 物理摘除层");
        assertTrue(BongAnimationPlayer.activeAnimations(playerId).isEmpty(), "断线必须清空 ACTIVE_LAYERS bookkeeping");
        assertEquals(0, BongAnimationPlayer.pendingRemovalsSize(), "断线必须清空 PENDING_REMOVALS bookkeeping");
    }

    @Test
    void disconnectIsIdempotentAndFreshSessionCanReplaySameAnimationOnNewStack() {
        AnimationStack oldStack = new AnimationStack();
        AnimationStack freshStack = new AnimationStack();

        assertTrue(BongAnimationPlayer.playOnStack(oldStack, playerId, WALK, 500, 0));
        BongAnimationPlayer.clearOnDisconnect();
        BongAnimationPlayer.clearOnDisconnect();

        assertEquals(0, layerCount(oldStack), "重复断线清理不得让旧层残留");
        assertTrue(BongAnimationPlayer.playOnStack(freshStack, playerId, WALK, 500, 0),
            "新 session 同 UUID+同动画必须安装到新的 AnimationStack");
        assertEquals(1, layerCount(freshStack), "fresh session 必须获得新 layer，而不是复用旧 stack 绑定");
        assertTrue(BongAnimationPlayer.activeAnimations(playerId).contains(WALK));
    }

    @Test
    void disconnectPreservesInjectedLocalPlayerPredicateSeam() {
        UUID localPlayer = UUID.randomUUID();
        Identifier base = new Identifier("bong_test", "disconnect_predicate");
        Identifier fpv = BongAnimationPlayer.fpvVariantId(base);
        BongAnimationRegistry.register(fpv, minimalAnimation());
        BongAnimationPlayer.setLocalPlayerPredicateForTest(localPlayer::equals);

        BongAnimationPlayer.clearOnDisconnect();

        BongAnimationPlayer.FpvResolution resolution = BongAnimationPlayer.resolveFpvContent(localPlayer, base);
        assertEquals(fpv, resolution.contentId(),
            "断线只可清 session data，不能复位长期 localPlayerPredicate 测试 seam");
        assertTrue(resolution.useFpvArms(), "保留 predicate 后，FPV 变体路径仍必须可用");
    }

    @Test
    void disconnectClearsChannelOwnershipAndFreshSessionCanUseSameChannel() {
        AnimationStack oldStack = new AnimationStack();
        AnimationStack freshStack = new AnimationStack();

        assertTrue(AnimationLayerManager.playOnStack(
            oldStack, playerId, AnimationLayerManager.Channel.UPPER_BODY, FIST, 0, 0
        ));
        assertEquals(FIST, AnimationLayerManager.activeInChannel(
            playerId, AnimationLayerManager.Channel.UPPER_BODY
        ));

        BongAnimationPlayer.clearOnDisconnect();
        AnimationLayerManager.clearOnDisconnect();

        assertNull(AnimationLayerManager.activeInChannel(playerId, AnimationLayerManager.Channel.UPPER_BODY),
            "断线必须清空 ACTIVE_BY_CHANNEL，不能把旧 session owner 带到新 stack");
        assertTrue(AnimationLayerManager.playOnStack(
            freshStack, playerId, AnimationLayerManager.Channel.UPPER_BODY, FIST, 0, 0
        ), "新 session 同 channel+同动画必须能重新播放");
        assertEquals(1, layerCount(freshStack));
    }

    @Test
    void disconnectDropsDelayedComboClosuresWithoutExecutingThem() {
        AtomicInteger executions = new AtomicInteger();
        BongPunchCombo.scheduleForTest(3, executions::incrementAndGet);
        assertEquals(1, BongPunchCombo.pendingActionsForTest(), "前置：延迟 closure 必须已排队");

        BongPunchCombo.clearOnDisconnect();
        BongPunchCombo.clearOnDisconnect();

        assertEquals(0, BongPunchCombo.pendingActionsForTest(), "断线必须幂等地清空 delayed closure 队列");
        BongPunchCombo.tickPendingForTest();
        assertEquals(0, executions.get(), "断线清理不能执行旧 player/world closure");

        BongPunchCombo.scheduleForTest(1, executions::incrementAndGet);
        BongPunchCombo.tickPendingForTest();
        assertEquals(1, executions.get(), "fresh session 排入的新 closure 必须仍可正常执行");
    }

    private static KeyframeAnimation minimalAnimation() {
        KeyframeAnimation.AnimationBuilder builder =
            new KeyframeAnimation.AnimationBuilder(AnimationFormat.UNKNOWN);
        builder.endTick = 1;
        builder.isLooped = false;
        return builder.build();
    }

    @SuppressWarnings("unchecked")
    private static int layerCount(AnimationStack stack) {
        try {
            Field field = AnimationStack.class.getDeclaredField("layers");
            field.setAccessible(true);
            return new ArrayList<>((List<Pair<Integer, IAnimation>>) field.get(stack)).size();
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError("无法读取 AnimationStack.layers 来核验物理移除", exception);
        }
    }
}
