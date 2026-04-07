package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;

public final class VisualEffectController {
    private VisualEffectController() {
    }

    public static VisualEffectState acceptIncoming(
        VisualEffectState currentState,
        VisualEffectState incomingState,
        long nowMillis,
        boolean enabled
    ) {
        VisualEffectState safeCurrentState = currentState == null ? VisualEffectState.none() : currentState;
        VisualEffectState safeIncomingState = incomingState == null ? VisualEffectState.none() : incomingState;
        if (!enabled || safeIncomingState.isEmpty()) {
            return safeCurrentState;
        }

        VisualEffectProfile profile = VisualEffectProfile.from(safeIncomingState);
        if (profile == null) {
            return safeCurrentState;
        }

        long safeNowMillis = Math.max(0L, nowMillis);
        if (isWithinRetriggerWindow(safeCurrentState, safeIncomingState, safeNowMillis, profile)) {
            return safeCurrentState;
        }

        return VisualEffectState.create(
            profile.effectType().wireName(),
            Math.min(profile.maxIntensity(), safeIncomingState.intensity()),
            Math.min(profile.maxDurationMillis(), safeIncomingState.durationMillis()),
            safeNowMillis
        );
    }

    static boolean isWithinRetriggerWindow(
        VisualEffectState currentState,
        VisualEffectState incomingState,
        long nowMillis,
        VisualEffectProfile profile
    ) {
        if (currentState == null || currentState.isEmpty() || incomingState == null || incomingState.isEmpty()) {
            return false;
        }
        if (currentState.effectType() != incomingState.effectType()) {
            return false;
        }

        long elapsedSinceTriggerMillis = Math.max(0L, nowMillis - Math.max(0L, currentState.startedAtMillis()));
        return elapsedSinceTriggerMillis < profile.retriggerWindowMillis();
    }
}
