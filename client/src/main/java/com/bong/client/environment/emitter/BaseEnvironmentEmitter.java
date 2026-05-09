package com.bong.client.environment.emitter;

import com.bong.client.environment.EmitterBehavior;
import com.bong.client.environment.EnvironmentEffect;
import com.bong.client.environment.EnvironmentParticleHelper;
import net.minecraft.util.math.Vec3d;

abstract class BaseEnvironmentEmitter implements EmitterBehavior {
    @Override
    public final void onTickInRadius(
        Vec3d playerPos,
        EnvironmentEffect effect,
        float deltaTick
    ) {
        EnvironmentParticleHelper.spawn(effect, 1.0f, deltaTick);
    }

    @Override
    public final void onTickInRadius(
        Vec3d playerPos,
        EnvironmentEffect effect,
        float alpha,
        float deltaTick
    ) {
        EnvironmentParticleHelper.spawn(effect, alpha, deltaTick);
    }
}
