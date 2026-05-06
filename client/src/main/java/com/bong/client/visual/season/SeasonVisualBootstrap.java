package com.bong.client.visual.season;

import com.bong.client.state.SeasonState;
import com.bong.client.state.SeasonStateStore;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.particle.ParticleTypes;
import net.minecraft.util.math.random.Random;

public final class SeasonVisualBootstrap {
    private SeasonVisualBootstrap() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(SeasonVisualBootstrap::onEndClientTick);
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.world == null || client.player == null) {
            return;
        }

        long worldTick = client.world.getTime();
        SeasonState state = SeasonStateStore.snapshot();
        SeasonVisuals.ParticleCue cue = SeasonVisuals.particleCue(state, worldTick);
        if (cue.kind() == SeasonVisuals.ParticleKind.NONE || cue.count() == 0) {
            return;
        }

        Random random = client.world.random;
        for (int i = 0; i < cue.count(); i++) {
            double x = client.player.getX() + (random.nextDouble() - 0.5) * 8.0;
            double y = client.player.getY() + 1.2 + random.nextDouble() * 1.6;
            double z = client.player.getZ() + (random.nextDouble() - 0.5) * 8.0;
            if (cue.kind() == SeasonVisuals.ParticleKind.SNOW_GRAIN) {
                client.world.addParticle(ParticleTypes.SNOWFLAKE, x, y, z, 0.0, cue.yVelocity(), 0.0);
            } else {
                client.world.addParticle(ParticleTypes.CLOUD, x, y, z, 0.0, cue.yVelocity(), 0.0);
            }
        }
    }
}
