package com.bong.client.environment;

import com.mojang.blaze3d.systems.RenderSystem;

public final class EnvironmentSkyController {
    private static float[] previousShaderColor;

    private EnvironmentSkyController() {
    }

    public static void applyBeforeSky() {
        EnvironmentFogCommand command = EnvironmentFogController.currentCommand();
        if (command == null) {
            return;
        }
        if (previousShaderColor == null) {
            previousShaderColor = RenderSystem.getShaderColor().clone();
        }
        int rgb = command.skyColorRgb();
        RenderSystem.setShaderColor(red(rgb), green(rgb), blue(rgb), 1.0f);
    }

    public static void resetAfterSky() {
        if (previousShaderColor == null || previousShaderColor.length < 4) {
            return;
        }
        RenderSystem.setShaderColor(
            previousShaderColor[0],
            previousShaderColor[1],
            previousShaderColor[2],
            previousShaderColor[3]
        );
        previousShaderColor = null;
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
