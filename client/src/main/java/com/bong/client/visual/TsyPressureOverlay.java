package com.bong.client.visual;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

/** TSY/负灵压 HUD 与相机反馈的纯计算入口。 */
public final class TsyPressureOverlay {
    private static final double WEAK_PRESSURE = 0.4;
    private static final double FULL_PRESSURE = 1.1;
    private static final double MAX_FOV_SHRINK_DEGREES = 4.0;

    private TsyPressureOverlay() {
    }

    public static List<HudRenderCommand> buildCommands(
        double localNegPressure,
        int screenWidth,
        int screenHeight
    ) {
        double intensity = pressureIntensity(localNegPressure);
        if (intensity <= 0.0 || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        int alpha = (int) Math.round(0x22 + intensity * 0x66);
        int color = (alpha << 24) | 0x5A1B88;
        List<HudRenderCommand> commands = new ArrayList<>(1);
        commands.add(HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, color));
        return List.copyOf(commands);
    }

    public static double fovOffsetDegrees(double localNegPressure) {
        double intensity = pressureIntensity(localNegPressure);
        if (intensity <= 0.0) {
            return 0.0;
        }
        return -MAX_FOV_SHRINK_DEGREES * intensity;
    }

    public static double pressureIntensity(double localNegPressure) {
        if (!Double.isFinite(localNegPressure) || localNegPressure >= 0.0) {
            return 0.0;
        }
        double pressure = -localNegPressure;
        if (pressure < WEAK_PRESSURE) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, (pressure - WEAK_PRESSURE) / (FULL_PRESSURE - WEAK_PRESSURE)));
    }
}
