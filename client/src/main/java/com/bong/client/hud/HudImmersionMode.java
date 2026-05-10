package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.state.VisualEffectState;

import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;

public final class HudImmersionMode {
    public static final long PEACE_AFTER_COMBAT_MS = 10_000L;
    public static final long CROSSFADE_MS = 300L;
    public static final long IMMERSIVE_FADE_OUT_MS = 500L;
    public static final long IMMERSIVE_FADE_IN_MS = 300L;
    public static final long ALT_PEEK_EXIT_MS = 3_000L;
    public static final long AUTO_MEDITATION_DELAY_MS = 3_000L;

    public enum Mode {
        PEACE,
        COMBAT,
        CULTIVATION
    }

    private static volatile long lastCombatAtMs = -1L;
    private static volatile Mode currentMode = Mode.PEACE;
    private static volatile long changedAtMs = 0L;
    private static volatile boolean manualImmersive = false;
    private static volatile boolean immersiveActive = false;
    private static volatile long immersiveChangedAtMs = -IMMERSIVE_FADE_IN_MS;
    private static volatile long altPeekStartedAtMs = -1L;
    private static volatile long meditationStartedAtMs = -1L;
    private static volatile boolean autoMeditationImmersive = true;
    private static final EnumSet<HudRenderLayer> VISIBLE_CULTIVATION = EnumSet.of(
        HudRenderLayer.BASELINE,
        HudRenderLayer.ZONE,
        HudRenderLayer.QI_RADAR,
        HudRenderLayer.THREAT_INDICATOR,
        HudRenderLayer.HUD_VARIANT,
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
        HudRenderLayer.COMPASS,
        HudRenderLayer.QI_RADAR,
        HudRenderLayer.HUD_VARIANT,
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
        if (combatState != null && combatState.active()) {
            lastCombatAtMs = safeNow;
            next = Mode.COMBAT;
        } else if (isMeditating(visualEffectState, safeNow)) {
            next = Mode.CULTIVATION;
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
        return Math.min(1.0, transitionElapsedMillis(nowMs) / (double) CROSSFADE_MS);
    }

    public static long transitionElapsedMillis(long nowMs) {
        return Math.max(0L, Math.max(0L, nowMs) - changedAtMs);
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
        manualImmersive = false;
        immersiveActive = false;
        immersiveChangedAtMs = -IMMERSIVE_FADE_IN_MS;
        altPeekStartedAtMs = -1L;
        meditationStartedAtMs = -1L;
        autoMeditationImmersive = true;
    }

    public static void toggleManual(long nowMs) {
        setManualImmersive(!manualImmersive, nowMs);
    }

    public static void setManualImmersive(boolean enabled, long nowMs) {
        if (manualImmersive != enabled) {
            long safeNow = Math.max(0L, nowMs);
            manualImmersive = enabled;
            immersiveActive = enabled;
            immersiveChangedAtMs = safeNow;
        }
    }

    public static boolean manualImmersive() {
        return manualImmersive;
    }

    static void setAutoMeditationImmersiveForTests(boolean enabled) {
        autoMeditationImmersive = enabled;
    }

    public static List<HudRenderCommand> applyImmersiveAlpha(
        List<HudRenderCommand> commands,
        Mode mode,
        VisualEffectState visualEffectState,
        HudRuntimeContext runtimeContext,
        long nowMs
    ) {
        if (commands == null || commands.isEmpty()) {
            return List.of();
        }
        long safeNow = Math.max(0L, nowMs);
        HudRuntimeContext runtime = runtimeContext == null ? HudRuntimeContext.empty() : runtimeContext;
        if (runtime.altPeekDown()) {
            if (altPeekStartedAtMs < 0L) {
                altPeekStartedAtMs = safeNow;
            } else if (manualImmersive && safeNow - altPeekStartedAtMs >= ALT_PEEK_EXIT_MS) {
                setManualImmersive(false, nowMs);
            }
        } else {
            altPeekStartedAtMs = -1L;
        }

        if (isMeditating(visualEffectState, safeNow)) {
            if (meditationStartedAtMs < 0L) {
                meditationStartedAtMs = safeNow;
            }
        } else {
            meditationStartedAtMs = -1L;
        }

        boolean autoMeditation = autoMeditationImmersive
            && meditationStartedAtMs >= 0L
            && safeNow - meditationStartedAtMs >= AUTO_MEDITATION_DELAY_MS;
        boolean combatRestore = mode == Mode.COMBAT;
        boolean active = (manualImmersive || autoMeditation) && !combatRestore;
        long transitionAtMs = active && autoMeditation && !manualImmersive
            ? meditationStartedAtMs + AUTO_MEDITATION_DELAY_MS
            : safeNow;
        updateImmersiveTransition(active, transitionAtMs);
        if (combatRestore) {
            return commands;
        }
        double alpha = immersiveAlpha(active, runtime.altPeekDown(), safeNow);
        if (alpha >= 0.999) {
            return commands;
        }
        List<HudRenderCommand> out = new ArrayList<>(commands.size());
        for (HudRenderCommand command : commands) {
            if (command != null) {
                out.add(criticalLayer(command.layer()) ? command : HudCommandAlpha.withAlpha(command, alpha));
            }
        }
        return List.copyOf(out);
    }

    static double immersiveAlpha(boolean active, boolean altPeekDown, long nowMs) {
        if (active && altPeekDown) {
            return 0.6;
        }
        long elapsed = Math.max(0L, Math.max(0L, nowMs) - immersiveChangedAtMs);
        if (active) {
            return 1.0 - Math.min(1.0, elapsed / (double) IMMERSIVE_FADE_OUT_MS);
        }
        if (elapsed >= IMMERSIVE_FADE_IN_MS) {
            return 1.0;
        }
        return Math.min(1.0, elapsed / (double) IMMERSIVE_FADE_IN_MS);
    }

    private static void updateImmersiveTransition(boolean active, long nowMs) {
        if (immersiveActive != active) {
            immersiveActive = active;
            immersiveChangedAtMs = Math.max(0L, nowMs);
        }
    }

    private static boolean criticalLayer(HudRenderLayer layer) {
        return layer == HudRenderLayer.THREAT_INDICATOR
            || layer == HudRenderLayer.EDGE_FEEDBACK
            || layer == HudRenderLayer.NEAR_DEATH
            || layer == HudRenderLayer.TSY_EXTRACT
            || layer == HudRenderLayer.REALM_COLLAPSE
            || layer == HudRenderLayer.HUD_VARIANT;
    }

    private static boolean isMeditating(VisualEffectState visualEffectState, long nowMs) {
        if (visualEffectState == null || !visualEffectState.isActiveAt(nowMs)) {
            return false;
        }
        return visualEffectState.effectType() == VisualEffectState.EffectType.MEDITATION_CALM
            || visualEffectState.effectType() == VisualEffectState.EffectType.MEDITATION_INK_WASH;
    }
}
