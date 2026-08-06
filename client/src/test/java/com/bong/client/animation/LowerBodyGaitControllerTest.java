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

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

class LowerBodyGaitControllerTest {
    private UUID playerId;

    @BeforeEach
    void setUp() {
        BongAnimationPlayer.resetForTest();
        AnimationLayerManager.resetForTest();
        LowerBodyGaitController.resetForTests();
        playerId = UUID.randomUUID();
        for (GaitSelector.Gait gait : GaitSelector.Gait.values()) {
            Identifier animId = gait.animId();
            if (animId != null) {
                BongAnimationRegistry.register(animId, minimalAnimation());
            }
        }
    }

    @AfterEach
    void tearDown() {
        LowerBodyGaitController.resetForTests();
        AnimationLayerManager.resetForTest();
        BongAnimationPlayer.resetForTest();
    }

    @Test
    void sameGaitTickDoesNotRestartOrAddAnotherLayer() {
        AnimationStack stack = new AnimationStack();

        LowerBodyGaitController.applyOnStack(stack, playerId, GaitSelector.Gait.WALK);
        LowerBodyGaitController.applyOnStack(stack, playerId, GaitSelector.Gait.WALK);

        assertEquals(1, layerCount(stack), "同一档位连续 tick 必须复用已有 LOWER_BODY 层");
        assertEquals(
            GaitSelector.Gait.WALK.animId(),
            AnimationLayerManager.activeInChannel(playerId, AnimationLayerManager.Channel.LOWER_BODY),
            "同一档位连续 tick 必须保留 WALK 通道所有权"
        );
        assertEquals(GaitSelector.Gait.WALK, LowerBodyGaitController.activeGaitForTests());
    }

    @Test
    void changedGaitReplacesThePreviousLowerBodyLayer() {
        AnimationStack stack = new AnimationStack();

        LowerBodyGaitController.applyOnStack(stack, playerId, GaitSelector.Gait.WALK);
        LowerBodyGaitController.applyOnStack(stack, playerId, GaitSelector.Gait.JOG);

        assertEquals(2, layerCount(stack), "档位切换淡出窗口内应同时保留旧层和新层");
        assertEquals(
            GaitSelector.Gait.JOG.animId(),
            AnimationLayerManager.activeInChannel(playerId, AnimationLayerManager.Channel.LOWER_BODY),
            "档位切换后通道所有权必须立即指向新档位"
        );
        assertEquals(GaitSelector.Gait.JOG, LowerBodyGaitController.activeGaitForTests());
        for (int i = 0; i < GaitSelector.Gait.WALK.fadeOutTicks() + 1; i++) {
            BongAnimationPlayer.tickPendingRemovalsForTest();
        }
        assertEquals(1, layerCount(stack), "旧档位淡出完成后 stack 只应保留新层");
    }

    @Test
    void noneStopsTheActiveLowerBodyLayer() {
        AnimationStack stack = new AnimationStack();

        LowerBodyGaitController.applyOnStack(stack, playerId, GaitSelector.Gait.WALK);
        LowerBodyGaitController.applyOnStack(stack, playerId, GaitSelector.Gait.NONE);

        assertEquals(1, layerCount(stack), "NONE 先执行淡出，旧层应暂留一个淡出窗口");
        assertNull(
            AnimationLayerManager.activeInChannel(playerId, AnimationLayerManager.Channel.LOWER_BODY),
            "NONE 后 LOWER_BODY 不得残留通道所有权"
        );
        assertEquals(GaitSelector.Gait.NONE, LowerBodyGaitController.activeGaitForTests());
    }

    @Test
    void failedPlaybackKeepsThePreviousControllerOwner() {
        AnimationStack stack = new AnimationStack();

        LowerBodyGaitController.applyOnStack(stack, playerId, GaitSelector.Gait.WALK);
        BongAnimationPlayer.resetForTest();
        LowerBodyGaitController.applyOnStack(stack, playerId, GaitSelector.Gait.DASH);

        assertEquals(GaitSelector.Gait.WALK, LowerBodyGaitController.activeGaitForTests(),
            "新步态播放失败时控制器必须保留可停止的旧档位状态");
        assertEquals(
            GaitSelector.Gait.WALK.animId(),
            AnimationLayerManager.activeInChannel(playerId, AnimationLayerManager.Channel.LOWER_BODY),
            "新步态播放失败时 LOWER_BODY ownership 不得被错误覆盖"
        );
    }

    @Test
    void replacementStackUpdatesControllerOwnerAfterNewPlaybackSucceeds() {
        AnimationStack oldStack = new AnimationStack();
        AnimationStack replacementStack = new AnimationStack();

        LowerBodyGaitController.applyOnStack(oldStack, playerId, GaitSelector.Gait.WALK);
        LowerBodyGaitController.applyOnStack(replacementStack, playerId, GaitSelector.Gait.JOG);

        assertEquals(GaitSelector.Gait.JOG, LowerBodyGaitController.activeGaitForTests(),
            "新 stack 播放成功后控制器 owner 必须切到新档位");
        assertEquals(
            GaitSelector.Gait.JOG.animId(),
            AnimationLayerManager.activeInChannel(playerId, AnimationLayerManager.Channel.LOWER_BODY),
            "新 stack 播放成功后 LOWER_BODY ownership 必须指向新动画"
        );
        for (int i = 0; i < GaitSelector.Gait.WALK.fadeOutTicks() + 1; i++) {
            BongAnimationPlayer.tickPendingRemovalsForTest();
        }
        assertEquals(0, layerCount(oldStack), "旧 stack 淡出完成后不得残留旧步态层");
        assertEquals(1, layerCount(replacementStack), "新 stack 必须保留当前步态层");
    }

    @Test
    void disconnectClearsControllerState() {
        AnimationStack stack = new AnimationStack();
        LowerBodyGaitController.applyOnStack(stack, playerId, GaitSelector.Gait.WALK);

        LowerBodyGaitController.clearOnDisconnect();

        assertEquals(GaitSelector.Gait.NONE, LowerBodyGaitController.activeGaitForTests(),
            "断线后控制器不得把旧玩家档位带入下一 session");
    }

    @Test
    void idleAfterStackReplacementStopsTheRecordedOldStack() {
        AnimationStack oldStack = new AnimationStack();
        AnimationStack replacementStack = new AnimationStack();

        LowerBodyGaitController.applyOnStack(oldStack, playerId, GaitSelector.Gait.WALK);
        LowerBodyGaitController.applyOnStack(replacementStack, playerId, GaitSelector.Gait.NONE);

        assertEquals(1, layerCount(oldStack), "淡出完成前旧 stack 层应暂留以完成平滑退出");
        assertEquals(0, layerCount(replacementStack), "idle 替换栈不应安装新步态层");
        assertNull(
            AnimationLayerManager.activeInChannel(playerId, AnimationLayerManager.Channel.LOWER_BODY),
            "idle 替换栈后 LOWER_BODY 不得残留旧通道所有权"
        );
        assertEquals(GaitSelector.Gait.NONE, LowerBodyGaitController.activeGaitForTests());
        for (int i = 0; i < GaitSelector.Gait.WALK.fadeOutTicks() + 1; i++) {
            BongAnimationPlayer.tickPendingRemovalsForTest();
        }
        assertEquals(0, layerCount(oldStack), "旧 stack 淡出完成后必须物理摘层");
        assertEquals(0, BongAnimationPlayer.pendingRemovalsSize(), "旧 stack 清理完成后不得留 pending");
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
            Object raw = field.get(stack);
            assertNotNull(raw, "AnimationStack.layers 必须存在");
            return new ArrayList<>((List<Pair<Integer, IAnimation>>) raw).size();
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError("无法读取 AnimationStack.layers", exception);
        }
    }
}
