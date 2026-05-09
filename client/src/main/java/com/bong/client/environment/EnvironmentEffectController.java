package com.bong.client.environment;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.util.math.Vec3d;

import java.util.Collection;

public final class EnvironmentEffectController {
    private static final double DEFAULT_RADIUS = 80.0;
    private static final EnvironmentEffectRegistry REGISTRY = new EnvironmentEffectRegistry();
    private static final EnvironmentAudioController AUDIO = new EnvironmentAudioController();
    private static boolean bootstrapped;

    private EnvironmentEffectController() {
    }

    public static void bootstrap() {
        if (bootstrapped) {
            return;
        }
        bootstrapped = true;
        REGISTRY.registerBuiltInBehaviors();
        ClientTickEvents.END_CLIENT_TICK.register(EnvironmentEffectController::tick);
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> clear());
    }

    public static void acceptPayload(String jsonPayload) {
        EnvironmentEffectParser.ParseResult result = EnvironmentEffectParser.parse(jsonPayload);
        if (!result.ok()) {
            BongClient.LOGGER.warn("Ignoring bong:zone_environment payload: {}", result.error());
            return;
        }
        acceptState(result.state());
    }

    public static void acceptState(ZoneEnvironmentState state) {
        MinecraftClient client = MinecraftClient.getInstance();
        String currentDimension = currentDimensionId(client);
        if (state != null && !state.matchesDimension(currentDimension)) {
            return;
        }
        REGISTRY.onZoneStateUpdate(state);
    }

    static EnvironmentEffectRegistry registryForTests() {
        return REGISTRY;
    }

    static void resetForTests() {
        clear();
        REGISTRY.registerBuiltInBehaviors();
    }

    private static void tick(MinecraftClient client) {
        ClientPlayerEntity player = client.player;
        if (client.world == null || player == null) {
            clear();
            return;
        }

        Vec3d playerPos = player.getPos();
        REGISTRY.tickFade(playerPos, DEFAULT_RADIUS);
        Collection<ActiveEmitter> active = REGISTRY.activeNearPlayer(playerPos, DEFAULT_RADIUS);
        AUDIO.update(active, playerPos);
        EnvironmentFogController.update(active, playerPos);
        for (ActiveEmitter emitter : active) {
            emitter.behavior().onTickInRadius(
                playerPos,
                emitter.effect(),
                emitter.alpha(),
                1.0f
            );
        }
    }

    private static String currentDimensionId(MinecraftClient client) {
        if (client == null || client.world == null) {
            return null;
        }
        return client.world.getRegistryKey().getValue().toString();
    }

    public static void clear() {
        REGISTRY.clear();
        AUDIO.clear();
        EnvironmentFogController.clear();
    }
}
