package com.bong.client.hud;

import com.bong.client.combat.store.StatusEffectStore;

import java.util.ArrayList;
import java.util.List;

/**
 * Top-center 8-slot status-effect strip (plan §U2 / §1 "HUD 顶部状态效果栏").
 * Icon border uses {@code source_color}; stacks &ge; 2 show ×N; remaining time
 * renders as a thin bottom progress bar.
 */
public final class StatusEffectHudPlanner {
    public static final int SLOT_SIZE = 18;
    public static final int SLOT_GAP = 3;
    public static final int TOP_MARGIN = 4;
    public static final int TRACK_BG = 0xC0101820;
    public static final int STACK_COLOR = 0xFFFFE080;

    private StatusEffectHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        List<StatusEffectStore.Effect> top = StatusEffectStore.topBar();
        if (top.isEmpty()) return out;
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        int totalWidth = top.size() * SLOT_SIZE + (top.size() - 1) * SLOT_GAP;
        int x = (screenWidth - totalWidth) / 2;
        int y = TOP_MARGIN;

        for (StatusEffectStore.Effect e : top) {
            // Border (2px frame in source color)
            int border = e.sourceColor();
            out.add(HudRenderCommand.rect(HudRenderLayer.STATUS_EFFECTS, x, y, SLOT_SIZE, SLOT_SIZE, border));
            // Inner background
            out.add(HudRenderCommand.rect(
                HudRenderLayer.STATUS_EFFECTS, x + 1, y + 1, SLOT_SIZE - 2, SLOT_SIZE - 2, TRACK_BG
            ));
            // Kind tint fill (dimmer)
            int tint = tintForKind(e.kind());
            out.add(HudRenderCommand.rect(
                HudRenderLayer.STATUS_EFFECTS, x + 2, y + 2, SLOT_SIZE - 4, SLOT_SIZE - 4, tint
            ));
            // Remaining-time bar (bottom 2px)
            long rem = e.remainingMs();
            float norm = remainingNorm(rem);
            int bar = Math.max(0, Math.round((SLOT_SIZE - 4) * norm));
            if (bar > 0) {
                out.add(HudRenderCommand.rect(
                    HudRenderLayer.STATUS_EFFECTS, x + 2, y + SLOT_SIZE - 3, bar, 1, 0xFFFFFFFF
                ));
            }
            // Stack count
            if (e.stacks() >= 2) {
                out.add(HudRenderCommand.text(
                    HudRenderLayer.STATUS_EFFECTS,
                    "\u00D7" + Math.min(99, e.stacks()),
                    x + SLOT_SIZE - 10,
                    y + SLOT_SIZE - 9,
                    STACK_COLOR
                ));
            }
            x += SLOT_SIZE + SLOT_GAP;
        }
        return out;
    }

    private static int tintForKind(StatusEffectStore.Kind kind) {
        return switch (kind) {
            case DOT -> 0x80E04040;
            case CONTROL -> 0x80B060FF;
            case BUFF -> 0x8060D060;
            case DEBUFF -> 0x80FFA030;
            case UNKNOWN -> 0x80808080;
        };
    }

    private static float remainingNorm(long remainingMs) {
        if (remainingMs <= 0L) return 0f;
        // Clamp to 30s; anything longer appears as full.
        float norm = remainingMs / 30_000f;
        if (norm > 1f) return 1f;
        if (norm < 0f) return 0f;
        return norm;
    }
}
