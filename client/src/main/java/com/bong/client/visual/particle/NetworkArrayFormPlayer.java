package com.bong.client.visual.particle;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioPosition;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.network.AudioEventPayload;
import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.List;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicLong;

public final class NetworkArrayFormPlayer implements VfxPlayer {
    public static final Identifier FORM = new Identifier("bong", "network_array_form");
    public static final Identifier BREAK = new Identifier("bong", "network_array_break");

    private static final int FORM_RGB = 0x96D6EC;
    private static final int BREAK_RGB = 0xD96666;
    private static final AtomicLong AUDIO_INSTANCE_ID = new AtomicLong(42_000L);

    private final Kind kind;

    public NetworkArrayFormPlayer(Kind kind) {
        this.kind = kind;
    }

    public enum Kind {
        FORM("network_array_form"),
        BREAK("network_array_break");

        private final String audioRecipeId;

        Kind(String audioRecipeId) {
            this.audioRecipeId = audioRecipeId;
        }
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
        float[] rgb = GameplayVfxUtil.rgb(payload, kind == Kind.FORM ? FORM_RGB : BREAK_RGB);
        int count = GameplayVfxUtil.count(payload, 3, 3, 4);
        int maxAge = GameplayVfxUtil.duration(payload, kind == Kind.FORM ? 30 : 20);
        double strength = GameplayVfxUtil.strength(payload, 0.65);

        if (kind == Kind.FORM) {
            spawnForm(client, world, ox, oy, oz, rgb, count, maxAge, strength);
        } else {
            spawnBreak(client, world, ox, oy, oz, rgb, count, maxAge);
        }

        SoundRecipePlayer.instance().play(new AudioEventPayload.PlaySoundRecipe(
            kind.audioRecipeId,
            AUDIO_INSTANCE_ID.incrementAndGet(),
            Optional.of(new AudioPosition((int) Math.floor(ox), (int) Math.floor(oy), (int) Math.floor(oz))),
            Optional.empty(),
            1.0f,
            0.0f,
            audioRecipe(kind)
        ));
    }

    private static void spawnForm(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        float[] rgb,
        int count,
        int maxAge,
        double strength
    ) {
        double radius = 2.15 + strength * 0.9;
        GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
            ox, oy + 0.02, oz, rgb, 0.58f, maxAge, radius * 0.72);
        for (int i = 0; i < count; i++) {
            double a0 = Math.PI * 2.0 * i / count - Math.PI / 2.0;
            double a1 = Math.PI * 2.0 * ((i + 1) % count) / count - Math.PI / 2.0;
            double x0 = ox + Math.cos(a0) * radius;
            double z0 = oz + Math.sin(a0) * radius;
            double x1 = ox + Math.cos(a1) * radius;
            double z1 = oz + Math.sin(a1) * radius;
            GameplayVfxUtil.spawnLine(
                client,
                world,
                BongParticles.qiAuraSprites,
                (x0 + x1) * 0.5,
                oy + 0.26,
                (z0 + z1) * 0.5,
                x1 - x0,
                0.0,
                z1 - z0,
                rgb,
                0.84f,
                maxAge,
                0.055
            );
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.runeCharSprites,
                x0,
                oy + 0.35,
                z0,
                0.0,
                0.035,
                0.0,
                rgb,
                0.72f,
                maxAge,
                0.16f
            );
        }
    }

    private static void spawnBreak(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        float[] rgb,
        int count,
        int maxAge
    ) {
        GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
            ox, oy + 0.02, oz, rgb, 0.45f, maxAge, 1.55);
        int burstCount = count * 4;
        for (int i = 0; i < burstCount; i++) {
            double theta = Math.PI * 2.0 * i / burstCount + (world.random.nextDouble() - 0.5) * 0.35;
            double speed = 0.08 + world.random.nextDouble() * 0.08;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.runeCharSprites,
                ox,
                oy + 0.22,
                oz,
                Math.cos(theta) * speed,
                0.02 + world.random.nextDouble() * 0.05,
                Math.sin(theta) * speed,
                rgb,
                0.66f,
                maxAge,
                0.14f
            );
            if (i < count) {
                GameplayVfxUtil.spawnLine(
                    client,
                    world,
                    BongParticles.qiAuraSprites,
                    ox,
                    oy + 0.28,
                    oz,
                    Math.cos(theta) * 1.5,
                    0.0,
                    Math.sin(theta) * 1.5,
                    rgb,
                    0.62f,
                    maxAge,
                    0.04
                );
            }
        }
    }

    static AudioRecipe audioRecipe(Kind kind) {
        return switch (kind) {
            case FORM -> new AudioRecipe(
                kind.audioRecipeId,
                List.of(
                    new AudioLayer(new Identifier("minecraft", "block.beacon.activate"), 0.5f, 1.3f, 0),
                    new AudioLayer(new Identifier("minecraft", "block.amethyst_block.chime"), 0.3f, 1.0f, 6)
                ),
                Optional.empty(),
                58,
                AudioAttenuation.WORLD_3D,
                AudioCategory.BLOCKS,
                AudioBus.ENVIRONMENT
            );
            case BREAK -> new AudioRecipe(
                kind.audioRecipeId,
                List.of(
                    new AudioLayer(new Identifier("minecraft", "block.beacon.deactivate"), 0.5f, 0.9f, 0),
                    new AudioLayer(new Identifier("minecraft", "block.glass.break"), 0.4f, 0.7f, 2)
                ),
                Optional.empty(),
                58,
                AudioAttenuation.WORLD_3D,
                AudioCategory.BLOCKS,
                AudioBus.ENVIRONMENT
            );
        };
    }
}
