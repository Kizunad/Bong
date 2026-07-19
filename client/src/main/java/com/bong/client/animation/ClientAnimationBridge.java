package com.bong.client.animation;

import com.bong.client.network.VfxEventAnimationBridge;
import dev.kosmx.playerAnim.api.layered.AnimationStack;
import dev.kosmx.playerAnim.minecraftApi.PlayerAnimationAccess;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.Identifier;

import java.util.Objects;
import java.util.OptionalInt;
import java.util.UUID;

/**
 * {@link VfxEventAnimationBridge} 生产实现：按 UUID 解析播放目标（当前
 * {@link ClientWorld} 里的在线玩家 → PlayerAnimator 层栈），然后派发到
 * {@link AnimationLayerManager}。
 *
 * <p>Phase 1 行为：
 * <ul>
 *   <li>目标玩家不在线（离视距 / 跨世界） → 返回 false，路由层降级 warn</li>
 *   <li>动画 id 未注册 → {@link AnimationLayerManager} 播放返回 false 透传</li>
 *   <li>fade ticks 缺省 → 走 {@code BongAnimationPlayer.DEFAULT_FADE_*_TICKS}</li>
 * </ul>
 *
 * <p>目标解析经 {@link AnimationTargetResolver} 接缝注入（默认 = MinecraftClient
 * 在线玩家解析）——headless 单测由此把真实 bridge 接到受控 {@link AnimationStack}
 * 上验证正向消费闭环（plan-skill-av-relink-v1 P3），解析之后的派发逻辑生产/测试
 * 共用同一条路径，无行为分叉。
 *
 * <p>Phase 2 扩展（视距过滤、广播）改这里即可，不动 schema。
 */
public final class ClientAnimationBridge implements VfxEventAnimationBridge {
    /** 动画播放目标：PlayerAnimator 层栈 + 玩家 UUID（生产 = 在线玩家的动画层）。 */
    public record AnimationTarget(AnimationStack stack, UUID playerId) {
        public AnimationTarget {
            Objects.requireNonNull(stack, "stack");
            Objects.requireNonNull(playerId, "playerId");
        }
    }

    /** UUID → 播放目标解析接缝。返回 null = 目标不可解析（bridge 返回 false 降级）。 */
    @FunctionalInterface
    public interface AnimationTargetResolver {
        AnimationTarget resolve(UUID playerUuid);
    }

    private final AnimationTargetResolver targetResolver;

    /** 生产构造：按 {@link MinecraftClient} 当前世界解析在线玩家。 */
    public ClientAnimationBridge() {
        this(ClientAnimationBridge::resolveOnlinePlayerTarget);
    }

    /** 注入构造（测试接缝）：被测对象仍是本类自身的派发逻辑，仅目标解析可控。 */
    public ClientAnimationBridge(AnimationTargetResolver targetResolver) {
        this.targetResolver = Objects.requireNonNull(targetResolver, "targetResolver");
    }

    @Override
    public boolean playAnim(
        UUID targetPlayer,
        Identifier animId,
        int priority,
        OptionalInt fadeInTicks
    ) {
        AnimationTarget target = targetResolver.resolve(targetPlayer);
        if (target == null) {
            return false;
        }
        int ticks = fadeInTicks.orElse(BongAnimationPlayer.DEFAULT_FADE_IN_TICKS);
        return AnimationLayerManager.playOnStack(
            target.stack(),
            target.playerId(),
            AnimationLayerManager.channelForPriority(priority),
            animId,
            priority,
            ticks,
            BongAnimationPlayer.DEFAULT_FADE_OUT_TICKS
        );
    }

    @Override
    public boolean playAnimInline(
        UUID targetPlayer,
        Identifier animId,
        String animJson,
        int priority,
        OptionalInt fadeInTicks
    ) {
        AnimationTarget target = targetResolver.resolve(targetPlayer);
        if (target == null) {
            return false;
        }
        int ticks = fadeInTicks.orElse(BongAnimationPlayer.DEFAULT_FADE_IN_TICKS);
        return BongAnimationRegistry.registerInlineJsonForPlayback(
            animId,
            animJson,
            () -> AnimationLayerManager.playOnStack(
                target.stack(),
                target.playerId(),
                AnimationLayerManager.channelForPriority(priority),
                animId,
                priority,
                ticks,
                BongAnimationPlayer.DEFAULT_FADE_OUT_TICKS
            )
        );
    }

    @Override
    public boolean stopAnim(
        UUID targetPlayer,
        Identifier animId,
        OptionalInt fadeOutTicks
    ) {
        AnimationTarget target = targetResolver.resolve(targetPlayer);
        if (target == null) {
            return false;
        }
        int ticks = fadeOutTicks.orElse(BongAnimationPlayer.DEFAULT_FADE_OUT_TICKS);
        return AnimationLayerManager.stopAnimationOnStack(
            target.stack(),
            target.playerId(),
            animId,
            ticks
        );
    }

    /** 生产目标解析：当前 {@link ClientWorld} 在线玩家 → PlayerAnimator 层栈。 */
    private static AnimationTarget resolveOnlinePlayerTarget(UUID uuid) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            return null;
        }
        ClientWorld world = client.world;
        if (world == null) {
            return null;
        }
        PlayerEntity entity = world.getPlayerByUuid(uuid);
        if (entity instanceof AbstractClientPlayerEntity clientPlayer) {
            return new AnimationTarget(
                PlayerAnimationAccess.getPlayerAnimLayer(clientPlayer),
                clientPlayer.getUuid()
            );
        }
        return null;
    }
}
