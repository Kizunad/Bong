package com.bong.client.visual.realm_vision;

import com.mojang.blaze3d.systems.RenderSystem;

public final class GlFogParamsSink implements FogParamsSink {
    @Override
    public void apply(RealmVisionCommand command) {
        if (command == null) return;
        RenderSystem.setShaderFogStart((float) command.fogStart());
        RenderSystem.setShaderFogEnd((float) command.fogEnd());
        RenderSystem.setShaderFogColor(red(command.fogColorRgb()), green(command.fogColorRgb()), blue(command.fogColorRgb()));
        RenderSystem.setShaderFogShape(command.fogShape() == FogShape.SPHERE ? net.minecraft.client.render.FogShape.SPHERE : net.minecraft.client.render.FogShape.CYLINDER);
    }

    private static float red(int rgb) { return ((rgb >>> 16) & 0xFF) / 255.0f; }
    private static float green(int rgb) { return ((rgb >>> 8) & 0xFF) / 255.0f; }
    private static float blue(int rgb) { return (rgb & 0xFF) / 255.0f; }
}
