package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * 腿伤减速触发时目标脚下血渍 decal（plan-combat-hit-location-v1 P3）。
 *
 * <p>复用既有 {@code lingqi_ripple} 环形贴图（无新资产）与
 * {@link BongGroundDecalParticle} 贴地基类，染深血红色区别于原本的青色灵纹用途。
 */
public final class LegWoundBloodDecalPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "combat_leg_wound_decal");

    private static final int FALLBACK_RGB = 0x8C1F1F;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }

        float[] rgb = GameplayVfxUtil.rgb(payload, FALLBACK_RGB);
        double strength = GameplayVfxUtil.strength(payload, 0.7);
        int maxAge = GameplayVfxUtil.duration(payload, 100);
        double halfSize = 0.3 + 0.25 * strength;
        float alpha = (float) Math.max(0.4, Math.min(0.9, strength));

        GameplayVfxUtil.spawnDecal(
            client,
            world,
            BongParticles.lingqiRippleSprites,
            payload.origin()[0],
            payload.origin()[1],
            payload.origin()[2],
            rgb,
            alpha,
            maxAge,
            halfSize
        );
    }
}
