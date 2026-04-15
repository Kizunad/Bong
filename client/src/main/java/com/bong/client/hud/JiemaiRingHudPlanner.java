package com.bong.client.hud;

import com.bong.client.combat.DefenseWindowState;

import java.util.ArrayList;
import java.util.List;

/**
 * Center-screen Jiemai parry ring (§3.2). Rendered while the
 * {@link DefenseWindowState} is active; the ring shrinks from OUTER_RADIUS to
 * INNER_RADIUS linearly over the defense window.
 *
 * <p>Since the existing render backend only supports rects, we approximate the
 * ring as 4 stacked rectangles (top/bottom/left/right), producing a diamond
 * that collapses to a point. Good enough for the conditional MVP.
 */
public final class JiemaiRingHudPlanner {
    public static final int OUTER_RADIUS = 60;
    public static final int INNER_RADIUS = 20;
    public static final int RING_COLOR = 0xFFFF4040;
    public static final int RING_THICKNESS = 2;

    private JiemaiRingHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        DefenseWindowState state,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.active()) return out;
        if (state.isExpired(nowMillis)) return out;
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        float progress = state.progress(nowMillis);
        int radius = (int) Math.round(OUTER_RADIUS + (INNER_RADIUS - OUTER_RADIUS) * progress);
        if (radius <= 0) return out;

        int cx = screenWidth / 2;
        int cy = screenHeight / 2;

        // Top
        out.add(HudRenderCommand.rect(
            HudRenderLayer.JIEMAI_RING,
            cx - radius,
            cy - radius,
            radius * 2,
            RING_THICKNESS,
            RING_COLOR
        ));
        // Bottom
        out.add(HudRenderCommand.rect(
            HudRenderLayer.JIEMAI_RING,
            cx - radius,
            cy + radius - RING_THICKNESS,
            radius * 2,
            RING_THICKNESS,
            RING_COLOR
        ));
        // Left
        out.add(HudRenderCommand.rect(
            HudRenderLayer.JIEMAI_RING,
            cx - radius,
            cy - radius,
            RING_THICKNESS,
            radius * 2,
            RING_COLOR
        ));
        // Right
        out.add(HudRenderCommand.rect(
            HudRenderLayer.JIEMAI_RING,
            cx + radius - RING_THICKNESS,
            cy - radius,
            RING_THICKNESS,
            radius * 2,
            RING_COLOR
        ));
        return out;
    }
}
