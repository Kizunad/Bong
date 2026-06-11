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

public final class LingjuActivatePlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "lingju_activate");

    private static final int FALLBACK_RGB = 0x7FD8A8;
    private static final String AUDIO_RECIPE_ID = "lingju_activate";
    private static final AtomicLong AUDIO_INSTANCE_ID = new AtomicLong(40_000L);

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
        int count = GameplayVfxUtil.count(payload, 8, 1, 8);
        int maxAge = GameplayVfxUtil.duration(payload, 20);

        for (int i = 0; i < count; i++) {
            double theta = (Math.PI * 2.0 * i) / count;
            double radius = 0.95;
            GameplayVfxUtil.spawnLine(
                client,
                world,
                BongParticles.qiAuraSprites,
                ox + Math.cos(theta) * radius,
                oy + 0.1,
                oz + Math.sin(theta) * radius,
                0.0,
                0.04,
                0.0,
                rgb,
                0.80f,
                maxAge,
                0.09
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
                new AudioLayer(new Identifier("minecraft", "block.amethyst_block.chime"), 0.6f, 0.8f, 0),
                new AudioLayer(new Identifier("minecraft", "block.amethyst_cluster.step"), 0.3f, 1.2f, 4)
            ),
            Optional.empty(),
            55,
            AudioAttenuation.WORLD_3D,
            AudioCategory.BLOCKS,
            AudioBus.ENVIRONMENT
        );
    }
}
