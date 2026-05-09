package com.bong.client.hud;

import com.bong.client.combat.store.VortexStateStore;

import java.util.List;

/** Small cooldown reminder; the skill slot itself still owns the grey mask. */
public final class VortexCooldownOverlay {
    private VortexCooldownOverlay() {
    }

    public static List<HudRenderCommand> buildCommands(
        VortexStateStore.State state,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        if (state == null || state.cooldownUntilMs() <= nowMillis) return List.of();
        long remainingMs = state.cooldownUntilMs() - nowMillis;
        long secondsLong = remainingMs / 1000L + (remainingMs % 1000L == 0L ? 0L : 1L);
        int seconds = (int) Math.min(Integer.MAX_VALUE, secondsLong);
        String text = "涡流 " + seconds + "s";
        int x = Math.max(8, (screenWidth - 56) / 2);
        int y = Math.max(16, screenHeight - 96);
        return List.of(HudRenderCommand.text(HudRenderLayer.VORTEX_COOLDOWN, text, x, y, 0xFFB8C8D8));
    }
}
