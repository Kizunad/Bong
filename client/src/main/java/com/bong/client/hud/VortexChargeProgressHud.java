package com.bong.client.hud;

import com.bong.client.combat.store.VortexStateStore;

import java.util.ArrayList;
import java.util.List;

/** Start-up / maintain progress strip for woliu-v2 casts. */
public final class VortexChargeProgressHud {
    static final int BAR_WIDTH = 118;
    static final int BAR_HEIGHT = 5;
    static final int TRACK_COLOR = 0xB0181420;
    static final int FILL_COLOR = 0xD060B8FF;

    private VortexChargeProgressHud() {
    }

    public static List<HudRenderCommand> buildCommands(
        VortexStateStore.State state,
        int screenWidth,
        int screenHeight
    ) {
        if (state == null || (!state.active() && state.chargeProgress() <= 0f)) return List.of();
        int x = (screenWidth - BAR_WIDTH) / 2;
        int y = Math.max(12, screenHeight - 82);
        int fill = Math.round(BAR_WIDTH * state.chargeProgress());

        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.VORTEX_CHARGE, x, y, BAR_WIDTH, BAR_HEIGHT, TRACK_COLOR));
        if (fill > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.VORTEX_CHARGE, x, y, fill, BAR_HEIGHT, FILL_COLOR));
        }
        if (!state.activeSkillId().isBlank()) {
            out.add(HudRenderCommand.text(
                HudRenderLayer.VORTEX_CHARGE,
                state.activeSkillId(),
                x,
                y - 10,
                0xFFE8F4FF
            ));
        }
        return out;
    }
}
