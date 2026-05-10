package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.state.VisualEffectState;

import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;

public final class HudImmersionMode {
    public static final long PEACE_AFTER_COMBAT_MS = 10_000L;
    public static final long CROSSFADE_MS = 300L;

    public enum Mode {
        PEACE,
        COMBAT,
        CULTIVATION
    }

    private static volatile long lastCombatAtMs = -1L;
    private static volatile Mode currentMode = Mode.PEACE;
    private static volatile long changedAtMs = 0L;
    private static final EnumSet<HudRenderLayer> VISIBLE_CULTIVATION = EnumSet.of(
        HudRenderLayer.BASELINE,
        HudRenderLayer.ZONE,
        HudRenderLayer.MINI_BODY,
        HudRenderLayer.PROCESSING_HUD,
        HudRenderLayer.EVENT_STREAM,
        HudRenderLayer.MERIDIAN_OPEN,
        HudRenderLayer.TOAST,
        HudRenderLayer.VISUAL,
        HudRenderLayer.LINGTIAN_OVERLAY,
        HudRenderLayer.REALM_COLLAPSE
    );
    private static final EnumSet<HudRenderLayer> VISIBLE_OTHER = EnumSet.of(
        HudRenderLayer.BASELINE,
        HudRenderLayer.ZONE,
        HudRenderLayer.TARGET_INFO,
        HudRenderLayer.MINI_BODY,
        HudRenderLayer.QUICK_BAR,
        HudRenderLayer.BOTANY,
        HudRenderLayer.PROCESSING_HUD,
        HudRenderLayer.EVENT_STREAM,
        HudRenderLayer.TOAST,
        HudRenderLayer.VISUAL,
        HudRenderLayer.LINGTIAN_OVERLAY
    );

    private HudImmersionMode() {
    }

    public static Mode resolve(CombatHudState combatState, VisualEffectState visualEffectState, long nowMs) {
        long safeNow = Math.max(0L, nowMs);
        Mode next;
        if (isMeditating(visualEffectState, safeNow)) {
            next = Mode.CULTIVATION;
        } else if (combatState != null && combatState.active()) {
            lastCombatAtMs = safeNow;
            next = Mode.COMBAT;
        } else if (lastCombatAtMs >= 0L
            && safeNow >= lastCombatAtMs
            && safeNow - lastCombatAtMs < PEACE_AFTER_COMBAT_MS) {
            next = Mode.COMBAT;
        } else {
            next = Mode.PEACE;
        }
        if (next != currentMode) {
            currentMode = next;
            changedAtMs = safeNow;
        }
        return next;
    }

    public static double transitionProgress(long nowMs) {
        long elapsed = Math.max(0L, Math.max(0L, nowMs) - changedAtMs);
        return Math.min(1.0, elapsed / (double) CROSSFADE_MS);
    }

    public static List<HudRenderCommand> filter(List<HudRenderCommand> commands, Mode mode) {
        if (commands == null || commands.isEmpty()) {
            return List.of();
        }
        EnumSet<HudRenderLayer> visible = switch (mode) {
            case COMBAT -> null;
            case CULTIVATION -> VISIBLE_CULTIVATION;
            case PEACE -> VISIBLE_OTHER;
        };
        List<HudRenderCommand> filtered = new ArrayList<>(commands.size());
        for (HudRenderCommand command : commands) {
            if (command != null && (visible == null || visible.contains(command.layer()))) {
                filtered.add(command);
            }
        }
        return List.copyOf(filtered);
    }

    public static void resetForTests() {
        lastCombatAtMs = -1L;
        currentMode = Mode.PEACE;
        changedAtMs = 0L;
    }

    private static boolean isMeditating(VisualEffectState visualEffectState, long nowMs) {
        if (visualEffectState == null || !visualEffectState.isActiveAt(nowMs)) {
            return false;
        }
        return visualEffectState.effectType() == VisualEffectState.EffectType.MEDITATION_CALM
            || visualEffectState.effectType() == VisualEffectState.EffectType.MEDITATION_INK_WASH;
    }
}
