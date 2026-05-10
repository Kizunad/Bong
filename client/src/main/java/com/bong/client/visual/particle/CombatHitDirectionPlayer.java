package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class CombatHitDirectionPlayer implements VfxPlayer {
    public static final Identifier HIT = new Identifier("bong", "combat_hit");
    public static final Identifier PARRY = new Identifier("bong", "combat_parry");

    private final boolean parry;

    public CombatHitDirectionPlayer(boolean parry) {
        this.parry = parry;
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        double[] dir = GameplayVfxUtil.direction(payload, new double[] { 1.0, 0.0, 0.0 });
        float[] rgb = GameplayVfxUtil.rgb(payload, parry ? 0x4488FF : 0xFF3344);
        int maxAge = GameplayVfxUtil.duration(payload, parry ? 16 : 12);

        if (parry) {
            int count = GameplayVfxUtil.count(payload, 8, 1, 18);
            for (int i = 0; i < count; i++) {
                double vx = (world.random.nextDouble() - 0.5) * 0.18;
                double vy = 0.04 + world.random.nextDouble() * 0.14;
                double vz = (world.random.nextDouble() - 0.5) * 0.18;
                GameplayVfxUtil.spawnSprite(client, world, BongParticles.tribulationSparkSprites,
                    ox, oy, oz, vx, vy, vz, rgb, 0.85f, maxAge, 0.10f);
            }
            return;
        }

        for (int i = -1; i <= 1; i++) {
            GameplayVfxUtil.spawnLine(
                client,
                world,
                BongParticles.swordSlashArcSprites,
                ox,
                oy + i * 0.08,
                oz,
                dir[0] * 0.7,
                0.05,
                dir[2] * 0.7,
                rgb,
                0.8f,
                maxAge,
                0.18
            );
        }
    }
}
