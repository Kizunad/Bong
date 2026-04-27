package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;

import java.util.OptionalInt;

/** Starts the client-local formation-core BlockEntity-style ground-decal demo. */
public final class FormationCoreDemoPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "formation_core_demo");

    private static final int FALLBACK_RGB = 0xC4E0FF;
    private static final int DEFAULT_DURATION_TICKS = 120;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }
        double[] origin = payload.origin();
        WorldVfxDemoBootstrap.spawnFormationCoreDemo(
            world,
            new Vec3d(origin[0], origin[1], origin[2]),
            payload.durationTicks().orElse(OptionalInt.of(DEFAULT_DURATION_TICKS).getAsInt()),
            payload.strength().orElse(0.9),
            payload.colorRgb().orElse(FALLBACK_RGB)
        );
    }
}
