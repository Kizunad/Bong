package com.bong.client.environment;

import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.math.Vec3d;

public interface EmitterBehavior {
    void onTickInRadius(MatrixStack stack, Vec3d playerPos, EnvironmentEffect effect, float deltaTick);

    default void onTickInRadius(
        MatrixStack stack,
        Vec3d playerPos,
        EnvironmentEffect effect,
        float alpha,
        float deltaTick
    ) {
        onTickInRadius(stack, playerPos, effect, deltaTick);
    }

    default String ambientLoopRecipe(EnvironmentEffect effect) {
        return effect.ambientLoopRecipe();
    }

    default int fadeInTicks() {
        return 40;
    }

    default int fadeOutTicks() {
        return 40;
    }
}
