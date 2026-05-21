package com.bong.client.npc;

/**
 * NPC 单个装备 slot 的展示数据（由 {@code bong:npc_metadata} 下发）。
 */
public record NpcEquipSlotData(
    String templateId,
    String displayName,
    int qualityTier,
    double durabilityRatio
) {
    public NpcEquipSlotData {
        templateId = templateId == null ? "" : templateId;
        displayName = displayName == null || displayName.isBlank() ? templateId : displayName;
        qualityTier = Math.max(0, Math.min(3, qualityTier));
        durabilityRatio = clamp01(durabilityRatio);
    }

    /**
     * 品质文字色（ARGB）。
     * 凡铁=0xC8C8C8, 灵器=0x5DD17A, 法宝=0x4A9EE0, 仙器=0xCDA4DE。
     */
    public int qualityColor() {
        return switch (qualityTier) {
            case 1 -> 0x5DD17A;
            case 2 -> 0x4A9EE0;
            case 3 -> 0xCDA4DE;
            default -> 0xC8C8C8;
        };
    }

    /** 品质中文标签。 */
    public String qualityLabel() {
        return switch (qualityTier) {
            case 1 -> "灵器";
            case 2 -> "法宝";
            case 3 -> "仙器";
            default -> "凡铁";
        };
    }

    /**
     * 耐久条色（RGB）。
     * > 0.5 绿(0x5DD17A) / 0.2-0.5 黄(0xE2C84A) / < 0.2 红(0xCC4444)。
     */
    public int durabilityColor() {
        if (durabilityRatio > 0.5) {
            return 0x5DD17A;
        }
        if (durabilityRatio >= 0.2) {
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
