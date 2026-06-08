package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class TiandaoHuntVfxPlayer implements VfxPlayer {
    public static final Identifier BEAST_SPAWN = new Identifier("bong", "tiandao_beast_spawn");
    public static final Identifier DIRECTED_THUNDER = new Identifier("bong", "tiandao_directed_thunder");

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        if (BEAST_SPAWN.equals(payload.eventId())) {
            playBeastSpawn(client, world, payload);
        } else if (DIRECTED_THUNDER.equals(payload.eventId())) {
            playDirectedThunder(client, world, payload);
        }
    }

    private static void playBeastSpawn(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        float[] decalRgb = GameplayVfxUtil.rgb(payload, 0x301808);
        int duration = GameplayVfxUtil.duration(payload, 100);
        GameplayVfxUtil.spawnDecal(
            client,
            world,
            BongParticles.lingqiRippleSprites,
            origin[0],
            origin[1] + 0.05,
            origin[2],
            decalRgb,
            0.45f,
            duration,
            1.5
        );

        float[] burstRgb = GameplayVfxUtil.rgb(payload, 0x604020);
        int count = GameplayVfxUtil.count(payload, 6, 1, 24);
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.04 + world.random.nextDouble() * 0.04;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.enlightenmentDustSprites,
                origin[0],
                origin[1] + 0.12,
                origin[2],
                Math.cos(angle) * speed,
                0.05 + world.random.nextDouble() * 0.04,
                Math.sin(angle) * speed,
                burstRgb,
                0.58f,
                20,
                0.1f
            );
        }
    }

    private static void playDirectedThunder(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        float[] thunderRgb = GameplayVfxUtil.rgb(payload, 0xE0E8FF);
        int lineCount = GameplayVfxUtil.count(payload, 3, 1, 8);
        for (int i = 0; i < lineCount; i++) {
            GameplayVfxUtil.spawnLine(
                client,
                world,
                BongParticles.tribulationSparkSprites,
                origin[0] + (world.random.nextDouble() - 0.5) * 6.0,
                origin[1] + 16.0 + world.random.nextDouble() * 8.0,
                origin[2] + (world.random.nextDouble() - 0.5) * 6.0,
                (world.random.nextDouble() - 0.5) * 0.4,
                -3.2 - world.random.nextDouble(),
                (world.random.nextDouble() - 0.5) * 0.4,
                thunderRgb,
                0.92f,
                5,
                0.18
            );
        }

        for (int i = 0; i < 15; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.07 + world.random.nextDouble() * 0.08;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.tribulationSparkSprites,
                origin[0],
                origin[1] + 0.2,
                origin[2],
                Math.cos(angle) * speed,
                0.08 + world.random.nextDouble() * 0.08,
                Math.sin(angle) * speed,
                new float[] { 1.0f, 0.88f, 0.63f },
                0.76f,
                15,
                0.14f
            );
        }

        GameplayVfxUtil.spawnDecal(
            client,
            world,
            BongParticles.qiAuraSprites,
            origin[0],
            origin[1] + 0.05,
            origin[2],
            new float[] { 0.13f, 0.06f, 0.03f },
            0.5f,
            200,
            3.0
        );
    }
}
