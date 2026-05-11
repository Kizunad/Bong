package com.bong.client.combat.juice;

import java.util.ArrayList;
import java.util.List;

public final class CombatJuiceCalibration {
    private CombatJuiceCalibration() {
    }

    public static List<PvpPairing> pvpPairings() {
        List<PvpPairing> out = new ArrayList<>(49);
        for (CombatSchool attacker : CombatSchool.playableSchools()) {
            for (CombatSchool defender : CombatSchool.playableSchools()) {
                CombatJuiceProfile critical = CombatJuiceProfile.select(attacker, CombatJuiceTier.CRITICAL);
                boolean sameColor = (attacker.qiColorArgb() & 0x00FFFFFF) == (defender.qiColorArgb() & 0x00FFFFFF);
                boolean inputLagRisk = critical.hitStopTicks() > 10;
                out.add(new PvpPairing(attacker, defender, sameColor, inputLagRisk, critical.hitStopTicks(), critical.shakeIntensity()));
            }
        }
        return List.copyOf(out);
    }

    public static PerformanceBudget mixedBattleBudget(int players, int minutes) {
        int safePlayers = Math.max(0, players);
        int safeMinutes = Math.max(0, minutes);
        int maxConcurrentJuiceEvents = safePlayers * 4;
        int estimatedFpsFloor = maxConcurrentJuiceEvents <= 20 ? 42 : Math.max(30, 42 - (maxConcurrentJuiceEvents - 20));
        return new PerformanceBudget(safePlayers, safeMinutes, maxConcurrentJuiceEvents, estimatedFpsFloor);
    }

    public record PvpPairing(
        CombatSchool attacker,
        CombatSchool defender,
        boolean sameQiColor,
        boolean inputLagRisk,
        int criticalHitStopTicks,
        float criticalShakeIntensity
    ) {
    }

    public record PerformanceBudget(
        int players,
        int minutes,
        int maxConcurrentJuiceEvents,
        int estimatedFpsFloor
    ) {
        public boolean passesPlanFloor() {
            return players >= 5 && minutes >= 10 && estimatedFpsFloor >= 30;
        }
    }
}
