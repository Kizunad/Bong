package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;

import java.util.OptionalInt;

/** Starts the client-local flying-sword entity/ribbon demo. */
public final class FlyingSwordDemoPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "flying_sword_demo");

    private static final int FALLBACK_RGB = 0x88CCFF;
    private static final int DEFAULT_DURATION_TICKS = 80;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }
        double[] origin = payload.origin();
        double[] direction = payload.direction().orElse(new double[] { 1.0, 0.0, 0.0 });
        WorldVfxDemoBootstrap.spawnFlyingSwordDemo(
            world,
            new Vec3d(origin[0], origin[1], origin[2]),
            new Vec3d(direction[0], direction[1], direction[2]),
            payload.durationTicks().orElse(OptionalInt.of(DEFAULT_DURATION_TICKS).getAsInt()),
            payload.strength().orElse(0.85),
            payload.colorRgb().orElse(FALLBACK_RGB)
        );
    }
}
