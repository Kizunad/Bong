package com.bong.client.tsy;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.List;

public final class TsyCorpseDeathVfx {
    private TsyCorpseDeathVfx() {
    }

    public static List<HudRenderCommand> buildCommands(TsyDeathVfxState state, long nowMillis, int screenWidth, int screenHeight) {
        if (state == null || !state.activeAt(nowMillis) || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        long age = Math.max(0L, nowMillis - state.startedAtMillis());
        int alpha = (int) Math.round(120.0 * (1.0 - age / 1_000.0));
        int tint = (Math.max(0, Math.min(120, alpha)) << 24) | 0x6F5B4A;
        return List.of(
            HudRenderCommand.screenTint(HudRenderLayer.VISUAL, tint),
            HudRenderCommand.text(HudRenderLayer.TARGET_INFO, "秘境所得尽数坠落", screenWidth / 2 - 54, screenHeight / 2 + 32, 0xFFD8C6A0)
        );
    }
}
