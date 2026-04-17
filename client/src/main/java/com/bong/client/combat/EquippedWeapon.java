package com.bong.client.combat;

/**
 * plan-weapon-v1 §8.2：客户端 runtime 的武器装备快照。
 * 由 {@code WeaponEquippedHandler} 从 server 推送的 {@code WeaponEquippedV1}
 * payload 解析而来，供 {@code WeaponHotbarHudPlanner} 绘制。
 */
public record EquippedWeapon(
    String slot,
    long instanceId,
    String templateId,
    String weaponKind,
    float durabilityCurrent,
    float durabilityMax,
    int qualityTier,
    String soulBondCharacterId,
    int soulBondLevel,
    float soulBondProgress
) {
    public float durabilityRatio() {
        return durabilityMax > 0f ? Math.max(0f, Math.min(1f, durabilityCurrent / durabilityMax)) : 0f;
    }

    public boolean hasSoulBond() {
        return soulBondCharacterId != null;
    }
}
