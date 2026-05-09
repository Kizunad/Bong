package com.bong.client.environment;

import com.mojang.blaze3d.systems.RenderSystem;
import net.minecraft.client.render.FogShape;
import net.minecraft.util.math.Vec3d;

import java.util.Collection;

public final class EnvironmentFogController {
    private static final Sink GL_SINK = new GlSink();
    private static volatile Sink sink = GL_SINK;
    private static volatile EnvironmentFogCommand current;

    private EnvironmentFogController() {
    }

    public static void update(Collection<ActiveEmitter> activeEmitters, Vec3d playerPos) {
        current = EnvironmentFogPlanner.plan(activeEmitters, playerPos);
    }

    public static void applyFog() {
        EnvironmentFogCommand command = current;
        if (command != null) {
            sink.apply(command);
        }
    }

    public static EnvironmentFogCommand currentCommand() {
        return current;
    }

    public static void clear() {
        current = null;
    }

    public static void setSinkForTests(Sink testSink) {
        sink = testSink == null ? GL_SINK : testSink;
    }

    public interface Sink {
        void apply(EnvironmentFogCommand command);
    }

    private static final class GlSink implements Sink {
        @Override
        public void apply(EnvironmentFogCommand command) {
            RenderSystem.setShaderFogStart((float) command.fogStart());
            RenderSystem.setShaderFogEnd((float) command.fogEnd());
            RenderSystem.setShaderFogColor(red(command.fogColorRgb()), green(command.fogColorRgb()), blue(command.fogColorRgb()));
            RenderSystem.setShaderFogShape(FogShape.CYLINDER);
        }
    }

    private static float red(int rgb) {
        return ((rgb >>> 16) & 0xFF) / 255.0f;
    }

    private static float green(int rgb) {
        return ((rgb >>> 8) & 0xFF) / 255.0f;
    }

    private static float blue(int rgb) {
        return (rgb & 0xFF) / 255.0f;
    }
}
