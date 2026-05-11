package com.bong.client.combat.juice;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudTextHelper;

import java.util.ArrayList;
import java.util.List;

public final class CombatJuiceHudPlanner {
    private CombatJuiceHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(long nowMs, int screenWidth, int screenHeight) {
        List<HudRenderCommand> out = new ArrayList<>();
        CombatJuiceSystem.Overlay overlay = CombatJuiceSystem.activeOverlay(nowMs);
        if (overlay.activeAt(nowMs)) {
            int color = overlay.colorAt(nowMs);
            out.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, color));
            if (overlay.vignette()) {
                out.add(HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, color));
            }
        }

        KillJuiceController.KillState kill = KillJuiceController.activeKill(nowMs);
        if (kill.activeAt(nowMs) && screenWidth > 0 && screenHeight > 0) {
            int alpha = Math.max(0, Math.min(255, (int) Math.round(255.0 * kill.remainingRatioAt(nowMs))));
            String label = "+1 " + kill.victimName();
            out.add(HudRenderCommand.text(
                HudRenderLayer.EVENT_STREAM,
                label,
                Math.max(8, screenWidth - 96),
                Math.max(24, screenHeight / 2 - 28),
                HudTextHelper.withAlpha(0xD8E4D0, alpha)
            ));
            KillJuiceController.MultiKillState multi = KillJuiceController.multiKill();
            if (multi.count() >= 2) {
                out.add(HudRenderCommand.scaledText(
                    HudRenderLayer.EVENT_STREAM,
                    "x" + multi.count(),
                    Math.max(8, screenWidth - 84),
                    Math.max(24, screenHeight / 2 - 44),
                    HudTextHelper.withAlpha(0xFFC040, alpha),
                    Math.min(1.8, 1.0 + multi.count() * 0.1)
                ));
            }
        }
        return List.copyOf(out);
    }
}
