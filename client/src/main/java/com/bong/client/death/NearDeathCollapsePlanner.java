package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

public final class NearDeathCollapsePlanner {
    static final int QI_COLOR = 0x88C84444;
    static final int MERIDIAN_COLOR = 0xB0FF4040;
    static final int SURFACE_COLOR = 0xAA5A0000;

    private NearDeathCollapsePlanner() {}

    public static List<HudRenderCommand> buildCommands(DeathCinematicState state, int width, int height) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active() || width <= 0 || height <= 0) return out;
        double progress = state.phaseProgress();
        out.add(HudRenderCommand.screenTint(HudRenderLayer.NEAR_DEATH, alpha(QI_COLOR, 40 + (int) (progress * 60))));
        out.add(HudRenderCommand.edgeVignette(HudRenderLayer.NEAR_DEATH, alpha(MERIDIAN_COLOR, 80 + (int) (progress * 70))));
        out.add(HudRenderCommand.text(
            HudRenderLayer.NEAR_DEATH,
            state.skipPredeath() ? "你已经习惯死亡" : "真元外泄 · 经脉承压 · 肉身将裂",
            Math.max(8, width / 2 - 76),
            height - 46,
            0xFFFFC0C0
        ));
        return out;
    }

    public static int qiEscapeDensityByHp(double hpPercent) {
        double clamped = Math.max(0.0, Math.min(1.0, hpPercent));
        if (clamped >= 0.20) return 0;
        return Math.max(1, (int) Math.ceil((1.0 - clamped) * 3.0));
    }

    public static boolean meridianGlowOnSevered(boolean hasSeveredMeridian, double hpPercent) {
        return hasSeveredMeridian || hpPercent < 0.10;
    }

    public static int surfaceCrackLines(double hpPercent) {
        return hpPercent < 0.05 ? 8 : 0;
    }

    public static boolean collapseFreezeBeforeDeath(long phaseTick) {
        return phaseTick >= 14L && phaseTick <= 20L;
    }

    private static int alpha(int color, int alpha) {
        return (Math.max(0, Math.min(255, alpha)) << 24) | (color & 0x00FFFFFF);
    }
}
