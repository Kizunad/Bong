package com.bong.client.inventory.model;

/**
 * plan-layered-equip-v1 P4（决议 #17）：装备槽类型，与 server EquipSlotV1 三端对齐。
 *
 * <p>身体槽 HEAD/CHEST/LEGS/FEET = worn（穿戴，可多层叠放，含护甲 / 伪皮 / 背包件）；
 * 手槽 MAIN_HAND/OFF_HAND/EXTRA_HAND_0/EXTRA_HAND_1 = held（持械，单件）。
 *
 * <p>P4 删除（决议 #17）：FALSE_SKIN（伪皮归 CHEST worn 层）、TWO_HAND（双手武器走 MAIN_HAND held + 锁 OFF_HAND）、
 * TREASURE_BELT_0..3（法宝激活态迁灵宝 UI 触发位，决议 #8）、BACK_PACK/WAIST_POUCH/CHEST_SATCHEL
 * （背包按 ContainerSpec.equip_slot 指向身体槽 worn 层，不再有专属槽）。
 * P4 新增（决议 #17）：EXTRA_HAND_0/EXTRA_HAND_1（多臂持械槽，server schema + proto 已有 extra_hand_0/1）。
 */
public enum EquipSlotType {
    HEAD("头甲"),
    CHEST("胸甲"),
    LEGS("腿甲"),
    FEET("足甲"),
    MAIN_HAND("右手"),
    OFF_HAND("左手"),
    EXTRA_HAND_0("臂三"),
    EXTRA_HAND_1("臂四");

    private final String displayName;

    EquipSlotType(String displayName) {
        this.displayName = displayName;
    }

    public String displayName() {
        return displayName;
    }

    /**
     * 该槽是否为手槽（持械 held 位）。手槽放武器/工具/盾/法宝，身体槽穿戴 worn 层。
     */
    public boolean isHand() {
        return switch (this) {
            case MAIN_HAND, OFF_HAND, EXTRA_HAND_0, EXTRA_HAND_1 -> true;
            default -> false;
        };
    }

    /**
     * plan-layered-equip-v1 P0.3（决议 #2）：该槽的 C2S 装备态 wire 值。
     * 手槽（MAIN_HAND/OFF_HAND/EXTRA_HAND_0/EXTRA_HAND_1）= "held"（持械），其余身体槽 = "worn"（穿戴）。
     */
    public String wireState() {
        return isHand() ? "held" : "worn";
    }
}
