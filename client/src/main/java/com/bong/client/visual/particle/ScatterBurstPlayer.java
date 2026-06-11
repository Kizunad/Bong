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

public final class ScatterBurstPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "scatter_burst");

    private static final int FALLBACK_RGB = 0xE8F0EE;
    private static final String AUDIO_RECIPE_ID = "scatter_burst";
    private static final AtomicLong AUDIO_INSTANCE_ID = new AtomicLong(41_000L);

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, FALLBACK_RGB);
        int count = GameplayVfxUtil.count(payload, 14, 1, 24);
        int maxAge = GameplayVfxUtil.duration(payload, 16);

        for (int i = 0; i < count; i++) {
            double theta = (Math.PI * 2.0 * i) / count + (world.random.nextDouble() - 0.5) * 0.25;
            double pitch = Math.toRadians((world.random.nextDouble() * 30.0) - 15.0);
            double speed = 0.08 + world.random.nextDouble() * 0.07;
            double horizontal = Math.cos(pitch) * speed;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.qiAuraSprites,
                ox,
                oy + 0.15,
                oz,
                Math.cos(theta) * horizontal,
                Math.sin(pitch) * speed,
                Math.sin(theta) * horizontal,
                rgb,
                0.72f,
                maxAge,
                0.15f
            );
        }

        SoundRecipePlayer.instance().play(new AudioEventPayload.PlaySoundRecipe(
            AUDIO_RECIPE_ID,
            AUDIO_INSTANCE_ID.incrementAndGet(),
            Optional.of(new AudioPosition((int) Math.floor(ox), (int) Math.floor(oy), (int) Math.floor(oz))),
            Optional.empty(),
            1.0f,
            0.0f,
            audioRecipe()
        ));
    }

    static AudioRecipe audioRecipe() {
        return new AudioRecipe(
            AUDIO_RECIPE_ID,
            List.of(
                new AudioLayer(new Identifier("minecraft", "block.glass.break"), 0.7f, 1.4f, 0),
                new AudioLayer(new Identifier("minecraft", "entity.breeze.idle_air"), 0.4f, 1.0f, 3)
            ),
            Optional.empty(),
            55,
            AudioAttenuation.WORLD_3D,
            AudioCategory.BLOCKS,
            AudioBus.ENVIRONMENT
        );
    }
}
