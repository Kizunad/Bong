package com.bong.client.hud;

import com.bong.client.combat.store.VortexStateStore;

import java.util.ArrayList;
import java.util.List;

/** Aggregates woliu-v2 HUD surfaces without owning gameplay state. */
public final class WoliuV2HudPlanner {
    private WoliuV2HudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        VortexStateStore.State state,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        VortexStateStore.State safeState = state == null ? VortexStateStore.State.NONE : state;
        if (screenWidth <= 0 || screenHeight <= 0) return List.of();

        List<HudRenderCommand> out = new ArrayList<>();
        out.addAll(VortexChargeProgressHud.buildCommands(safeState, screenWidth, screenHeight));
        out.addAll(VortexCooldownOverlay.buildCommands(safeState, screenWidth, screenHeight, nowMillis));
        out.addAll(BackfireWarningHud.buildCommands(safeState, screenWidth, screenHeight));
        out.addAll(TurbulenceFieldVisualizeHud.buildCommands(safeState, screenWidth, screenHeight, nowMillis));
        return List.copyOf(out);
    }
}
