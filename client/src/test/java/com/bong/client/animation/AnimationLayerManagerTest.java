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
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class AnimationLayerManagerTest {
    private static final Identifier IDLE = new Identifier("bong_test", "idle_breathe");
    private static final Identifier FIST = new Identifier("bong_test", "fist_punch");
    private static final Identifier PALM = new Identifier("bong_test", "palm_strike");
    private static final Identifier WALK = new Identifier("bong_test", "walk");
    private static final Identifier BREAKTHROUGH = new Identifier("bong_test", "breakthrough");
    private static final Identifier MISSING = new Identifier("bong_test", "missing");

    private UUID playerId;

    @BeforeEach
    void setUp() {
        BongAnimationPlayer.resetForTest();
        AnimationLayerManager.resetForTest();
        playerId = UUID.randomUUID();
        for (Identifier id : List.of(IDLE, FIST, PALM, WALK, BREAKTHROUGH)) {
            BongAnimationRegistry.register(id, buildMinimalAnim());
        }
    }

    @AfterEach
    void tearDown() {
        BongAnimationPlayer.resetForTest();
        AnimationLayerManager.resetForTest();
    }

    @Test
    void sameChannelReplacesPreviousAnimation() {
        AnimationStack stack = new AnimationStack();

        assertTrue(AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, FIST, 0, 0
        ));
        assertTrue(AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, PALM, 0, 0
        ));

        assertEquals(PALM, AnimationLayerManager.activeInChannel(
            playerId, AnimationLayerManager.Channel.UPPER_BODY
        ));
        assertTrue(BongAnimationPlayer.activeAnimations(playerId).contains(PALM));
        assertEquals(1, layersIn(stack).size(), "同一语义层替换后只保留新层");
    }

    @Test
    void failedReplacementClearsStoppedChannelState() {
        AnimationStack stack = new AnimationStack();

        assertTrue(AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, FIST, 0, 0
        ));

        assertFalse(AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, MISSING, 0, 0
        ));

        assertNull(AnimationLayerManager.activeInChannel(
            playerId, AnimationLayerManager.Channel.UPPER_BODY
        ));
        assertEquals(0, layersIn(stack).size(), "新动画不存在时旧层已停止，通道状态也必须清空");
    }

    @Test
    void failedPreviousStopKeepsChannelState() {
        AnimationStack stack = new AnimationStack();

        assertTrue(AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, FIST, 0, 0
        ));
        BongAnimationPlayer.resetForTest();

        assertFalse(AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, PALM, 0, 0
        ));

        assertEquals(FIST, AnimationLayerManager.activeInChannel(
            playerId, AnimationLayerManager.Channel.UPPER_BODY
        ));
        assertEquals(1, layersIn(stack).size(), "旧动画未成功停止时不能覆盖通道追踪");
    }

    @Test
    void differentChannelsCoexistAndKeepPriorityOrder() {
        AnimationStack stack = new AnimationStack();

        AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.EXPRESSION, IDLE, 0, 0
        );
        AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.LOWER_BODY, WALK, 0, 0
        );
        AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, FIST, 0, 0
        );
        AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.FULL_BODY, BREAKTHROUGH, 0, 0
        );

        List<Pair<Integer, IAnimation>> layers = layersIn(stack);
        assertEquals(4, layers.size());
        assertEquals(AnimationLayerManager.Channel.EXPRESSION.priority(), (int) layers.get(0).getLeft());
        assertEquals(AnimationLayerManager.Channel.LOWER_BODY.priority(), (int) layers.get(1).getLeft());
        assertEquals(AnimationLayerManager.Channel.UPPER_BODY.priority(), (int) layers.get(2).getLeft());
        assertEquals(AnimationLayerManager.Channel.FULL_BODY.priority(), (int) layers.get(3).getLeft());
    }

    @Test
    void stopChannelDoesNotTouchOtherChannels() {
        AnimationStack stack = new AnimationStack();

        AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.LOWER_BODY, WALK, 0, 0
        );
        AnimationLayerManager.playOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, FIST, 0, 0
        );

        assertTrue(AnimationLayerManager.stopOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, 0
        ));

        assertEquals(WALK, AnimationLayerManager.activeInChannel(
            playerId, AnimationLayerManager.Channel.LOWER_BODY
        ));
        assertEquals(1, layersIn(stack).size());
    }

    private static KeyframeAnimation buildMinimalAnim() {
        KeyframeAnimation.AnimationBuilder builder =
            new KeyframeAnimation.AnimationBuilder(AnimationFormat.UNKNOWN);
        builder.endTick = 1;
        builder.isLooped = false;
        return builder.build();
    }

    @SuppressWarnings("unchecked")
    private static List<Pair<Integer, IAnimation>> layersIn(AnimationStack stack) {
        try {
            Field field = AnimationStack.class.getDeclaredField("layers");
            field.setAccessible(true);
            Object raw = field.get(stack);
            assertNotNull(raw);
            return new ArrayList<>((List<Pair<Integer, IAnimation>>) raw);
        } catch (ReflectiveOperationException ex) {
            throw new AssertionError("反射 AnimationStack.layers 失败", ex);
        }
    }
}
