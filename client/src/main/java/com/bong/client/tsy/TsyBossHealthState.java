package com.bong.client.tsy;

public record TsyBossHealthState(
    boolean active,
    String bossName,
    String realm,
    double healthRatio,
    int phase,
    int maxPhase,
    long updatedAtMillis
) {
    public TsyBossHealthState {
        bossName = bossName == null || bossName.isBlank() ? "秘境守灵" : bossName.trim();
        realm = realm == null || realm.isBlank() ? "未知" : realm.trim();
        healthRatio = clamp01(healthRatio);
        maxPhase = Math.max(1, Math.min(5, maxPhase));
        phase = Math.max(1, Math.min(maxPhase, phase));
        updatedAtMillis = Math.max(0L, updatedAtMillis);
    }

    public static TsyBossHealthState empty() {
        return new TsyBossHealthState(false, "", "", 0.0, 1, 1, 0L);
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
