package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.store.DerivedAttrsStore;

import java.util.ArrayList;
import java.util.List;

/**
 * Thin peak-highlight overlay on the真元条 (plan §U2 / §1 "真元条（战斗扩展）").
 * Reads {@code throughput_peak_norm} from {@link DerivedAttrsStore}.
 *
 * <p>We do not reimplement the main qi bar — we assume it is rendered elsewhere
 * (MiniBodyHudPlanner). This planner only draws the peak highlight strip.
 */
public final class ThroughputPeakHudPlanner {
    public static final int BAR_WIDTH = 120;
    public static final int BAR_HEIGHT = 2;
    public static final int BOTTOM_MARGIN = 50;
    public static final int COLOR = 0xFFFFE080;

    private ThroughputPeakHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        CombatHudState state,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active()) return out;
        DerivedAttrsStore.State ds = DerivedAttrsStore.snapshot();
        float peak = ds.throughputPeakNorm();
        if (peak <= 0.01f) return out;
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        int x = (screenWidth - BAR_WIDTH) / 2;
        int y = screenHeight - BAR_HEIGHT - BOTTOM_MARGIN;
        int markerX = x + Math.round(Math.min(1f, peak) * BAR_WIDTH) - 1;
        out.add(HudRenderCommand.rect(HudRenderLayer.STAMINA_BAR, markerX, y - 2, 2, BAR_HEIGHT + 4, COLOR));
        return out;
    }
}
