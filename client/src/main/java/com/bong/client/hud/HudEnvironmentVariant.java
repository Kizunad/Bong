package com.bong.client.hud;

import com.bong.client.state.ZoneState;
import com.bong.client.tsy.ExtractState;

public enum HudEnvironmentVariant {
    NORMAL,
    NEGATIVE_QI,
    DEAD_ZONE,
    TSY;

    public static HudEnvironmentVariant from(ZoneState zoneState, ExtractState extractState) {
        ZoneState zone = zoneState == null ? ZoneState.empty() : zoneState;
        if (isTsy(zone, extractState)) {
            return TSY;
        }
        if (zone.collapsed() || (zone.spiritQiRaw() == 0.0 && zone.spiritQiNormalized() == 0.0 && !zone.isEmpty())) {
            return DEAD_ZONE;
        }
        if (zone.negativeSpiritQi()) {
            return NEGATIVE_QI;
        }
        return NORMAL;
    }

    public static double boundaryLerp(ZoneState zoneState) {
        ZoneState zone = zoneState == null ? ZoneState.empty() : zoneState;
        if (zone.isEmpty()) {
            return 0.0;
        }
        if (zone.negativeSpiritQi() || zone.collapsed()) {
            return 1.0;
        }
        return 1.0 - Math.min(1.0, Math.max(0.0, zone.spiritQiNormalized()) / 0.15);
    }

    private static boolean isTsy(ZoneState zone, ExtractState extractState) {
        if (extractState != null && (extractState.hasActivePortal() || extractState.collapseActive(System.currentTimeMillis()))) {
            return true;
        }
        String zoneId = zone == null ? "" : zone.zoneId().toLowerCase(java.util.Locale.ROOT);
        return zoneId.contains("tsy") || zoneId.contains("tianshuiyao") || zoneId.contains("collapse");
    }
}
