package com.bong.client.atmosphere;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.List;

public final class ZoneAtmosphereHudPlanner {
    private ZoneAtmosphereHudPlanner() {
    }

    public static void append(List<HudRenderCommand> out, ZoneAtmosphereCommand command) {
        if (out == null || command == null) {
            return;
        }
        if (command.desaturation() > 0.0) {
            int alpha = (int) Math.round(96.0 * command.desaturation());
            out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, (alpha << 24) | 0xC8C8C8));
        }
        if (command.negativeZoneVisual() && command.vignetteIntensity() > 0.0) {
            out.add(HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, command.vignetteArgb()));
        }
        if (command.cameraShakeIntensity() > 0.0 || command.hardClipVoid()) {
            int alpha = command.cameraShakeIntensity() > 0.45 ? 0xD0 : 0x55;
            out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, (alpha << 24)));
        }
    }
}
