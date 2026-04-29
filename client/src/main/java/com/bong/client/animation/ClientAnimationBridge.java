package com.bong.client.animation;

import com.bong.client.network.VfxEventAnimationBridge;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;
import java.util.UUID;

/**
 * {@link VfxEventAnimationBridge} 生产实现：按 UUID 在当前 {@link ClientWorld} 里找玩家，
 * 然后派发到 {@link BongAnimationPlayer}。
 *
 * <p>Phase 1 行为：
 * <ul>
 *   <li>目标玩家不在线（离视距 / 跨世界） → 返回 false，路由层降级 warn</li>
 *   <li>动画 id 未注册 → {@link BongAnimationPlayer#play} 返回 false 透传</li>
 *   <li>fade ticks 缺省 → 走 {@code BongAnimationPlayer.DEFAULT_FADE_*_TICKS}</li>
 * </ul>
 *
 * <p>Phase 2 扩展（视距过滤、广播）改这里即可，不动 schema。
 */
public final class ClientAnimationBridge implements VfxEventAnimationBridge {
    @Override
    public boolean playAnim(
        UUID targetPlayer,
        Identifier animId,
        int priority,
        OptionalInt fadeInTicks
    ) {
        AbstractClientPlayerEntity player = resolvePlayer(targetPlayer);
        if (player == null) {
            return false;
        }
        int ticks = fadeInTicks.orElse(BongAnimationPlayer.DEFAULT_FADE_IN_TICKS);
        return BongAnimationPlayer.play(player, animId, priority, ticks);
    }

    @Override
    public boolean playAnimInline(
        UUID targetPlayer,
        Identifier animId,
        String animJson,
        int priority,
        OptionalInt fadeInTicks
    ) {
        if (!BongAnimationRegistry.registerInlineJson(animId, animJson)) {
            return false;
        }
        return playAnim(targetPlayer, animId, priority, fadeInTicks);
    }

    @Override
    public boolean stopAnim(
        UUID targetPlayer,
        Identifier animId,
        OptionalInt fadeOutTicks
    ) {
        AbstractClientPlayerEntity player = resolvePlayer(targetPlayer);
        if (player == null) {
            return false;
        }
        int ticks = fadeOutTicks.orElse(BongAnimationPlayer.DEFAULT_FADE_OUT_TICKS);
        return BongAnimationPlayer.stop(player, animId, ticks);
    }

    private static AbstractClientPlayerEntity resolvePlayer(UUID uuid) {
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
            return clientPlayer;
        }
        return null;
    }
}
