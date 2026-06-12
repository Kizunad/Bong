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

public final class ZhenfaActionVfxPlayer implements VfxPlayer {
    public static final Identifier TRAP = new Identifier("bong", "zhenfa_trap");
    public static final Identifier WARD = new Identifier("bong", "zhenfa_ward");
    public static final Identifier DEPLETE = new Identifier("bong", "zhenfa_deplete");
    public static final Identifier BEAST_TRAP_SNAP = new Identifier("bong", "beast_trap_snap");
    public static final Identifier TRIP_WIRE_TRIGGER = new Identifier("bong", "trip_wire_trigger");

    private static final AtomicLong AUDIO_INSTANCE_ID = new AtomicLong(43_000L);

    private final Kind kind;

    public ZhenfaActionVfxPlayer(Kind kind) {
        this.kind = kind;
    }

    public enum Kind {
        TRAP,
        WARD,
        DEPLETE,
        BITE_SNAP,
        TRIP_WIRE
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
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb());
        int maxAge = GameplayVfxUtil.duration(payload, kind == Kind.WARD ? 60 : 30);
        if (kind == Kind.BITE_SNAP) {
            spawnBiteSnap(client, world, ox, oy, oz, rgb, maxAge);
            playAudio(ox, oy, oz, "beast_trap_snap", beastTrapSnapRecipe());
            return;
        }
        double halfSize = kind == Kind.WARD ? 1.8 : 1.1;
        GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
            ox, oy, oz, rgb, 0.75f, maxAge, halfSize);
        int count = GameplayVfxUtil.count(payload, kind == Kind.WARD ? 20 : 12, 1, 48);
        for (int i = 0; i < count; i++) {
            double theta = world.random.nextDouble() * Math.PI * 2.0;
            double speed = kind == Kind.DEPLETE ? 0.03 : 0.10;
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.runeCharSprites,
                ox, oy + 0.2, oz,
                Math.cos(theta) * speed,
                0.02 + world.random.nextDouble() * 0.08,
                Math.sin(theta) * speed,
                rgb, 0.65f, maxAge, 0.18f);
        }
        if (kind == Kind.TRIP_WIRE) {
            playAudio(ox, oy, oz, "trip_wire_trigger", tripWireTriggerRecipe());
        }
    }

    private int fallbackRgb() {
        return switch (kind) {
            case TRAP -> 0xFF3344;
            case WARD -> 0x4488FF;
            case DEPLETE -> 0x888888;
            case BITE_SNAP -> 0xB06338;
            case TRIP_WIRE -> 0x66BBFF;
        };
    }

    private static void spawnBiteSnap(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        float[] rgb,
        int maxAge
    ) {
        GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
            ox, oy + 0.03, oz, rgb, 0.70f, maxAge, 0.85);
        for (int i = 0; i < 18; i++) {
            double theta = (Math.PI * 2.0 * i) / 18.0;
            double speed = 0.16 + world.random.nextDouble() * 0.06;
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.runeCharSprites,
                ox, oy + 0.18, oz,
                Math.cos(theta) * speed,
                0.03 + world.random.nextDouble() * 0.04,
                Math.sin(theta) * speed,
                rgb, 0.75f, Math.min(maxAge, 24), 0.16f);
        }
    }

    private static void playAudio(double ox, double oy, double oz, String recipeId, AudioRecipe recipe) {
        SoundRecipePlayer.instance().play(new AudioEventPayload.PlaySoundRecipe(
            recipeId,
            AUDIO_INSTANCE_ID.incrementAndGet(),
            Optional.of(new AudioPosition((int) Math.floor(ox), (int) Math.floor(oy), (int) Math.floor(oz))),
            Optional.empty(),
            1.0f,
            0.0f,
            recipe
        ));
    }

    static AudioRecipe beastTrapSnapRecipe() {
        return new AudioRecipe(
            "beast_trap_snap",
            List.of(
                new AudioLayer(new Identifier("minecraft", "block.chain.break"), 0.65f, 0.75f, 0),
                new AudioLayer(new Identifier("minecraft", "entity.wolf.hurt"), 0.35f, 1.20f, 1)
            ),
            Optional.empty(),
            65,
            AudioAttenuation.WORLD_3D,
            AudioCategory.HOSTILE,
            AudioBus.COMBAT
        );
    }

    static AudioRecipe tripWireTriggerRecipe() {
        return new AudioRecipe(
            "trip_wire_trigger",
            List.of(
                new AudioLayer(new Identifier("minecraft", "block.tripwire.click_on"), 0.55f, 0.95f, 0),
                new AudioLayer(new Identifier("minecraft", "block.note_block.hat"), 0.25f, 1.35f, 1)
            ),
            Optional.empty(),
            55,
            AudioAttenuation.WORLD_3D,
            AudioCategory.BLOCKS,
            AudioBus.ENVIRONMENT
        );
    }
}
