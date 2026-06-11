package com.bong.client.combat;

/**
 * plan-shield-block-v1 §P3：客户端盾牌装备快照（off_hand 槽专用）。
 *
 * <p>语义上盾牌不是武器，独立于 {@link EquippedWeapon}，由 {@link EquippedShieldStore}
 * 持有。{@code WeaponHotbarHudPlanner} 的 off_hand 槽读此 store，渲染盾牌耐久条。
 */
public record EquippedShield(
    long instanceId,
    String templateId,
    float durabilityCurrent,
    float durabilityMax
) {
    public float durabilityRatio() {
        return durabilityMax > 0f ? Math.max(0f, Math.min(1f, durabilityCurrent / durabilityMax)) : 0f;
    }
}
