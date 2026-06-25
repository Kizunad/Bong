package com.bong.client.inventory.model;

public enum EquipSlotType {
    HEAD("头甲"),
    CHEST("胸甲"),
    LEGS("腿甲"),
    FEET("足甲"),
    FALSE_SKIN("伪皮"),
    MAIN_HAND("右手"),
    OFF_HAND("左手"),
    TWO_HAND("双手"),
    TREASURE_BELT_0("宝1"),
    TREASURE_BELT_1("宝2"),
    TREASURE_BELT_2("宝3"),
    TREASURE_BELT_3("宝4"),
    BACK_PACK("背包"),
    WAIST_POUCH("腰包"),
    CHEST_SATCHEL("前挂");

    private final String displayName;

    EquipSlotType(String displayName) {
        this.displayName = displayName;
    }

    public String displayName() {
        return displayName;
    }

    /**
     * plan-layered-equip-v1 P0.3（决议 #2）：该槽的 C2S 装备态 wire 值。
     * 手槽（MAIN_HAND/OFF_HAND/TWO_HAND）= "held"（持械），其余身体槽 = "worn"（穿戴）。
     * 注：枚举废变体清理 + extra_hand 新增归 P4 面板重构；此 helper 仅供 PR-1 C2S state 必填使用。
     */
    public String wireState() {
        return switch (this) {
            case MAIN_HAND, OFF_HAND, TWO_HAND -> "held";
            default -> "worn";
        };
    }
}
