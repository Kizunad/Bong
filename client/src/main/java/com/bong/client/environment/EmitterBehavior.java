package com.bong.client.environment;

import net.minecraft.util.math.Vec3d;

public interface EmitterBehavior {
    void onTickInRadius(Vec3d playerPos, EnvironmentEffect effect, float deltaTick);

    default void onTickInRadius(
        Vec3d playerPos,
        EnvironmentEffect effect,
        float alpha,
        float deltaTick
    ) {
        onTickInRadius(playerPos, effect, deltaTick);
    }

    default String ambientLoopRecipe(EnvironmentEffect effect) {
        return effect.ambientLoopRecipe();
    }

    default int fadeInTicks(EnvironmentEffect effect) {
        return effect == null ? 40 : effect.fadeInTicks();
    }

    default int fadeOutTicks(EnvironmentEffect effect) {
        return effect == null ? 40 : effect.fadeOutTicks();
    }
}
