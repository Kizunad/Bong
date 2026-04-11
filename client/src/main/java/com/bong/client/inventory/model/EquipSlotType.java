package com.bong.client.inventory.model;

public enum EquipSlotType {
    HEAD("头部"),
    CHEST("护甲"),
    LEGS("腿甲"),
    FEET("鞋"),
    MAIN_HAND("右手"),
    OFF_HAND("左手"),
    TWO_HAND("双手");

    private final String displayName;

    EquipSlotType(String displayName) {
        this.displayName = displayName;
    }

    public String displayName() {
        return displayName;
    }
}
