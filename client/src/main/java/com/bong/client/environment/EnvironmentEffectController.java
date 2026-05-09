package com.bong.client.environment;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderContext;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents;
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
        WorldRenderEvents.AFTER_ENTITIES.register(EnvironmentEffectController::render);
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
    }

    private static void render(WorldRenderContext context) {
        MinecraftClient client = MinecraftClient.getInstance();
        ClientPlayerEntity player = client.player;
        if (client.world == null || player == null || context.matrixStack() == null) {
            return;
        }

        Vec3d playerPos = player.getPos();
        Collection<ActiveEmitter> active = REGISTRY.activeNearPlayer(playerPos, DEFAULT_RADIUS);
        for (ActiveEmitter emitter : active) {
            emitter.behavior().onTickInRadius(
                context.matrixStack(),
                playerPos,
                emitter.effect(),
                emitter.alpha(),
                context.tickDelta()
            );
        }
    }

    public static void clear() {
        REGISTRY.clear();
        AUDIO.clear();
        EnvironmentFogController.clear();
    }
}
