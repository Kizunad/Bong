package com.bong.client.combat;

/**
 * Tunables exposed by §8 &amp; §C8 for future wiring into the external config
 * file. Defaults match the plan; swap-ability is the primary goal here.
 */
public final class HudConfig {
    public static volatile double quickSlotCastInterruptMovement = CastInterruptRules.MOVEMENT_THRESHOLD_METERS;
    public static volatile double quickSlotCastInterruptContam = CastInterruptRules.CONTAM_RATE_PER_MAXHP_PER_SECOND;
    public static volatile boolean eventStreamVisible = true;
    public static volatile int hudScalePercent = 100;

    private HudConfig() {
    }

    public static void resetToDefaults() {
        quickSlotCastInterruptMovement = CastInterruptRules.MOVEMENT_THRESHOLD_METERS;
        quickSlotCastInterruptContam = CastInterruptRules.CONTAM_RATE_PER_MAXHP_PER_SECOND;
        eventStreamVisible = true;
        hudScalePercent = 100;
    }
}
