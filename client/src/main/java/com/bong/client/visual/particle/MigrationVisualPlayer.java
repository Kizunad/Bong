package com.bong.client.visual.particle;

import com.bong.client.BongClientFeatures;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.network.VfxEventPayload;
import com.bong.client.season.MigrationVisualPlanner;
import com.bong.client.state.VisualEffectState;
import com.bong.client.visual.VisualEffectController;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.particle.ParticleTypes;
import net.minecraft.util.Identifier;

public final class MigrationVisualPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "migration_visual");

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        MigrationVisualPlanner.MigrationVisualEvent event =
            MigrationVisualPlanner.fromVfxPayload(payload, world.getTime());
        MigrationVisualPlanner.MigrationVisualCommand command =
            MigrationVisualPlanner.plan(event, world.getTime());
        if (command.dustPerEntityPerFiveTicks() <= 0) {
            return;
        }
        triggerScreenShake(command.cameraShakeIntensity(), event.durationTicks());

        double[] origin = payload.origin();
        double[] direction = payload.direction().orElse(new double[] { event.directionX(), 0.0, event.directionZ() });
        int count = Math.min(64, Math.max(4, command.dustPerEntityPerFiveTicks() * Math.max(1, event.entityCount() / 6)));
        for (int i = 0; i < count; i++) {
            double spread = 2.0 + world.random.nextDouble() * 6.0;
            double side = (world.random.nextDouble() - 0.5) * 5.0;
            double x = origin[0] - direction[0] * spread + direction[2] * side;
            double y = origin[1] + 0.2 + world.random.nextDouble() * 1.0;
            double z = origin[2] - direction[2] * spread - direction[0] * side;
            world.addParticle(
                ParticleTypes.CLOUD,
                x,
                y,
                z,
                direction[0] * 0.035,
                0.01 + command.fogDensityDelta() * 0.04,
                direction[2] * 0.035
            );
        }
    }

    private static void triggerScreenShake(double intensity, int durationTicks) {
        long now = System.currentTimeMillis();
        long durationMillis = Math.min(1_200L, Math.max(250L, durationTicks * 50L / 4L));
        VisualEffectState incoming =
            VisualEffectState.create("screen_shake", intensity, durationMillis, now);
        if (incoming.isEmpty()) {
            return;
        }
        BongHudStateSnapshot current = BongHudStateStore.snapshot();
        VisualEffectState next = VisualEffectController.acceptIncoming(
            current.visualEffectState(),
            incoming,
            now,
            BongClientFeatures.ENABLE_VISUAL_EFFECTS
        );
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            current.zoneState(),
            current.narrationState(),
            next
        ));
    }
}
