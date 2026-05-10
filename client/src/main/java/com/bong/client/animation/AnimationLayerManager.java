package com.bong.client.animation;

import dev.kosmx.playerAnim.api.layered.AnimationStack;
import dev.kosmx.playerAnim.minecraftApi.PlayerAnimationAccess;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.util.Identifier;

import java.util.EnumMap;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

/**
 * Bong 语义层到 PlayerAnimator priority 的薄适配层。
 *
 * <p>{@link BongAnimationPlayer} 负责单条 animId 的播放/停止；本类只负责"同一语义层同一时间
 * 只能有一条当前动画"这个契约。不同语义层仍然共存在同一个 {@link AnimationStack} 里，由
 * PlayerAnimator 的 priority 排序决定覆盖关系。
 */
public final class AnimationLayerManager {
    private static final Map<UUID, EnumMap<Channel, Identifier>> ACTIVE_BY_CHANNEL = new HashMap<>();

    private AnimationLayerManager() {
    }

    public enum Channel {
        /** 头部/呼吸/轻量 idle；用合法最低网络 priority，避免落到 schema 下界以下。 */
        EXPRESSION(100),
        /** 下身步态、跛行、跑动。 */
        LOWER_BODY(500),
        /** 上身攻击、采集、施法。 */
        UPPER_BODY(1000),
        /** 打坐、倒地、突破等全身动作。 */
        FULL_BODY(3000);

        private final int priority;

        Channel(int priority) {
            this.priority = priority;
        }

        public int priority() {
            return priority;
        }
    }

    public static synchronized boolean playOnStack(
        AnimationStack stack,
        UUID playerId,
        Channel channel,
        Identifier animId
    ) {
        return playOnStack(
            stack,
            playerId,
            channel,
            animId,
            channel.priority(),
            BongAnimationPlayer.DEFAULT_FADE_IN_TICKS,
            BongAnimationPlayer.DEFAULT_FADE_OUT_TICKS
        );
    }

    public static boolean play(
        AbstractClientPlayerEntity player,
        Identifier animId,
        int priority,
        int fadeInTicks
    ) {
        if (player == null || animId == null) {
            return false;
        }
        return playOnStack(
            PlayerAnimationAccess.getPlayerAnimLayer(player),
            player.getUuid(),
            channelForPriority(priority),
            animId,
            priority,
            fadeInTicks,
            BongAnimationPlayer.DEFAULT_FADE_OUT_TICKS
        );
    }

    public static boolean stop(AbstractClientPlayerEntity player, Identifier animId, int fadeOutTicks) {
        if (player == null || animId == null) {
            return false;
        }
        return stopAnimationOnStack(
            PlayerAnimationAccess.getPlayerAnimLayer(player),
            player.getUuid(),
            animId,
            fadeOutTicks
        );
    }

    public static Channel channelForPriority(int priority) {
        if (priority >= Channel.FULL_BODY.priority()) {
            return Channel.FULL_BODY;
        }
        if (priority >= Channel.UPPER_BODY.priority()) {
            return Channel.UPPER_BODY;
        }
        if (priority >= Channel.LOWER_BODY.priority()) {
            return Channel.LOWER_BODY;
        }
        return Channel.EXPRESSION;
    }

    static synchronized boolean playOnStack(
        AnimationStack stack,
        UUID playerId,
        Channel channel,
        Identifier animId,
        int fadeInTicks,
        int fadeOutTicks
    ) {
        return playOnStack(
            stack,
            playerId,
            channel,
            animId,
            channel.priority(),
            fadeInTicks,
            fadeOutTicks
        );
    }

    static synchronized boolean playOnStack(
        AnimationStack stack,
        UUID playerId,
        Channel channel,
        Identifier animId,
        int priority,
        int fadeInTicks,
        int fadeOutTicks
    ) {
        if (stack == null || playerId == null || channel == null || animId == null) {
            return false;
        }
        EnumMap<Channel, Identifier> byChannel =
            ACTIVE_BY_CHANNEL.computeIfAbsent(playerId, unused -> new EnumMap<>(Channel.class));
        Identifier previous = byChannel.get(channel);
        if (previous != null && !previous.equals(animId)) {
            boolean stopped = BongAnimationPlayer.stopOnStack(
                stack, playerId, previous, Math.max(0, fadeOutTicks)
            );
            if (!stopped) {
                return false;
            }
            byChannel.remove(channel);
        }
        boolean played = BongAnimationPlayer.playOnStack(
            stack,
            playerId,
            animId,
            priority,
            Math.max(0, fadeInTicks)
        );
        if (played) {
            byChannel.put(channel, animId);
        } else if (byChannel.isEmpty()) {
            ACTIVE_BY_CHANNEL.remove(playerId);
        }
        return played;
    }

    static synchronized boolean stopAnimationOnStack(
        AnimationStack stack,
        UUID playerId,
        Identifier animId,
        int fadeOutTicks
    ) {
        if (stack == null || playerId == null || animId == null) {
            return false;
        }
        EnumMap<Channel, Identifier> byChannel = ACTIVE_BY_CHANNEL.get(playerId);
        boolean stopped = BongAnimationPlayer.stopOnStack(
            stack, playerId, animId, Math.max(0, fadeOutTicks)
        );
        if (stopped && byChannel != null) {
            byChannel.entrySet().removeIf(entry -> animId.equals(entry.getValue()));
            if (byChannel.isEmpty()) {
                ACTIVE_BY_CHANNEL.remove(playerId);
            }
        }
        return stopped;
    }

    static synchronized boolean stopOnStack(
        AnimationStack stack,
        UUID playerId,
        Channel channel,
        int fadeOutTicks
    ) {
        if (stack == null || playerId == null || channel == null) {
            return false;
        }
        EnumMap<Channel, Identifier> byChannel = ACTIVE_BY_CHANNEL.get(playerId);
        if (byChannel == null) {
            return false;
        }
        Identifier active = byChannel.get(channel);
        if (active == null) {
            return false;
        }
        boolean stopped = BongAnimationPlayer.stopOnStack(
            stack, playerId, active, Math.max(0, fadeOutTicks)
        );
        if (stopped) {
            byChannel.remove(channel);
            if (byChannel.isEmpty()) {
                ACTIVE_BY_CHANNEL.remove(playerId);
            }
        }
        return stopped;
    }

    static synchronized Identifier activeInChannel(UUID playerId, Channel channel) {
        EnumMap<Channel, Identifier> byChannel = ACTIVE_BY_CHANNEL.get(playerId);
        return byChannel == null ? null : byChannel.get(channel);
    }

    static synchronized void resetForTest() {
        ACTIVE_BY_CHANNEL.clear();
    }
}
