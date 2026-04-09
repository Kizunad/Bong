package com.bong.client.hud;

import com.bong.client.state.NarrationState;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;

public final class BongHudStateSnapshot {
    private final ZoneState zoneState;
    private final NarrationState narrationState;
    private final VisualEffectState visualEffectState;

    private BongHudStateSnapshot(ZoneState zoneState, NarrationState narrationState, VisualEffectState visualEffectState) {
        this.zoneState = zoneState;
        this.narrationState = narrationState;
        this.visualEffectState = visualEffectState;
    }

    public static BongHudStateSnapshot empty() {
        return new BongHudStateSnapshot(ZoneState.empty(), NarrationState.empty(), VisualEffectState.none());
    }

    public static BongHudStateSnapshot create(ZoneState zoneState, NarrationState narrationState, VisualEffectState visualEffectState) {
        return new BongHudStateSnapshot(
            zoneState == null ? ZoneState.empty() : zoneState,
            narrationState == null ? NarrationState.empty() : narrationState,
            visualEffectState == null ? VisualEffectState.none() : visualEffectState
        );
    }

    public ZoneState zoneState() {
        return zoneState;
    }

    public NarrationState narrationState() {
        return narrationState;
    }

    public VisualEffectState visualEffectState() {
        return visualEffectState;
    }

    public boolean isEmpty() {
        return zoneState.isEmpty() && narrationState.isEmpty() && visualEffectState.isEmpty();
    }
}
