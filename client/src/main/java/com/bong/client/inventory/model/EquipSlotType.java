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
    TREASURE_BELT_3("宝4");

    private final String displayName;

    EquipSlotType(String displayName) {
        this.displayName = displayName;
    }

    public String displayName() {
        return displayName;
    }
}
