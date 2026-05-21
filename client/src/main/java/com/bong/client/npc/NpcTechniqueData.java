package com.bong.client.npc;

/**
 * NPC 单条功法数据（由 {@code bong:npc_metadata} 下发）。
 */
public record NpcTechniqueData(
    String id,
    String displayName,
    double proficiency
) {
    public NpcTechniqueData {
        id = id == null ? "" : id;
        displayName = displayName == null || displayName.isBlank() ? id : displayName;
        proficiency = clamp01(proficiency);
    }

    /**
     * 熟练度条色（RGB）。
     * < 0.3 红(0xCC4444) / 0.3-0.6 黄(0xE2C84A) / > 0.6 绿(0x5DD17A)。
     */
    public int proficiencyColor() {
        if (proficiency > 0.6) {
            return 0x5DD17A;
        }
        if (proficiency >= 0.3) {
            return 0xE2C84A;
        }
        return 0xCC4444;
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
