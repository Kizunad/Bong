package com.bong.client.combat.juice;

public record CombatJuiceEvent(
    Kind kind,
    CombatSchool school,
    CombatJuiceTier tier,
    String attackerUuid,
    String targetUuid,
    String localPlayerUuid,
    String victimName,
    double directionX,
    double directionZ,
    boolean rareDrop,
    long receivedAtMs
) {
    public CombatJuiceEvent {
        kind = kind == null ? Kind.HIT : kind;
        school = school == null ? CombatSchool.GENERIC : school;
        tier = tier == null ? CombatJuiceTier.LIGHT : tier;
        attackerUuid = attackerUuid == null ? "" : attackerUuid;
        targetUuid = targetUuid == null ? "" : targetUuid;
        localPlayerUuid = localPlayerUuid == null ? "" : localPlayerUuid;
        victimName = victimName == null ? "" : victimName;
        if (!Double.isFinite(directionX)) {
            directionX = 0.0;
        }
        if (!Double.isFinite(directionZ)) {
            directionZ = 1.0;
        }
        receivedAtMs = Math.max(0L, receivedAtMs);
    }

    public static CombatJuiceEvent hit(
        CombatSchool school,
        CombatJuiceTier tier,
        String attackerUuid,
        String targetUuid,
        double directionX,
        double directionZ,
        long nowMs
    ) {
        return new CombatJuiceEvent(Kind.HIT, school, tier, attackerUuid, targetUuid, "", "", directionX, directionZ, false, nowMs);
    }

    public boolean localPlayerIsAttacker() {
        return !localPlayerUuid.isBlank() && localPlayerUuid.equals(attackerUuid);
    }

    public enum Kind {
        HIT,
        QI_COLLISION,
        FULL_CHARGE,
        OVERLOAD,
        PARRY,
        PERFECT_PARRY,
        DODGE,
        WOUND,
        KILL
    }
}
