package com.bong.client.environment;

import com.mojang.blaze3d.systems.RenderSystem;

public final class EnvironmentSkyController {
    private EnvironmentSkyController() {
    }

    public static void applyBeforeSky() {
        EnvironmentFogCommand command = EnvironmentFogController.currentCommand();
        if (command == null) {
            return;
        }
        int rgb = command.skyColorRgb();
        RenderSystem.setShaderColor(red(rgb), green(rgb), blue(rgb), 1.0f);
    }

    public static void resetAfterSky() {
        RenderSystem.setShaderColor(1.0f, 1.0f, 1.0f, 1.0f);
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
