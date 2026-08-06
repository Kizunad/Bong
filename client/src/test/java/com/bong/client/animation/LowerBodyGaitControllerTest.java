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
