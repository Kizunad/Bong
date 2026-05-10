package com.bong.client.atmosphere;

import com.bong.client.state.SeasonState;
import com.bong.client.state.ZoneState;

public record ZoneAtmosphereContext(
    ZoneState zoneState,
    SeasonState seasonState,
    int tsyTier,
    int collapseRemainingTicks,
    int collapseTotalTicks,
    ZoneAtmosphereProfile boundaryTarget,
    double boundaryProgress
) {
    public ZoneAtmosphereContext {
        zoneState = zoneState == null ? ZoneState.empty() : zoneState;
        tsyTier = Math.max(0, tsyTier);
        collapseRemainingTicks = Math.max(0, collapseRemainingTicks);
        collapseTotalTicks = Math.max(collapseRemainingTicks, collapseTotalTicks);
        boundaryProgress = ZoneAtmosphereProfile.clamp01(boundaryProgress);
    }

    public static ZoneAtmosphereContext of(ZoneState zoneState, SeasonState seasonState) {
        return new ZoneAtmosphereContext(zoneState, seasonState, 0, 0, 0, null, 0.0);
    }

    public ZoneAtmosphereContext withTsyTier(int tier) {
        return new ZoneAtmosphereContext(
            zoneState,
            seasonState,
            tier,
            collapseRemainingTicks,
            collapseTotalTicks,
            boundaryTarget,
            boundaryProgress
        );
    }

    public ZoneAtmosphereContext withCollapse(int remainingTicks, int totalTicks) {
        return new ZoneAtmosphereContext(
            zoneState,
            seasonState,
            tsyTier,
            remainingTicks,
            totalTicks,
            boundaryTarget,
            boundaryProgress
        );
    }

    public ZoneAtmosphereContext withBoundary(ZoneAtmosphereProfile target, double progress) {
        return new ZoneAtmosphereContext(
            zoneState,
            seasonState,
            tsyTier,
            collapseRemainingTicks,
            collapseTotalTicks,
            target,
            progress
        );
    }
}
