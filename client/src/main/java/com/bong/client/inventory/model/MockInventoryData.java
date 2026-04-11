package com.bong.client.inventory.model;

public final class MockInventoryData {
    private MockInventoryData() {}

    public static InventoryModel create() {
        return InventoryModel.builder()
            // Grid items — mixture of sizes
            .gridItem(
                InventoryItem.create("spirit_grass", "灵草", 1, 1, 0.5, "common", "低阶灵草，可入药炼丹"),
                0, 0
            )
            .gridItem(
                InventoryItem.create("broken_artifact", "破碎法宝", 2, 2, 3.0, "rare", "残破的上古法器，仍有微弱灵力波动"),
                1, 0
            )
            .gridItem(
                InventoryItem.create("poison_needle", "毒蛊飞针", 1, 2, 0.8, "uncommon", "淬毒骨针三枚，沾之即蚀经脉"),
                0, 2
            )
            .gridItem(
                InventoryItem.create("guyuan_pill", "固元丹", 1, 1, 0.2, "rare", "温补真元，服后可加速恢复灵力"),
                0, 4
            )
            .gridItem(
                InventoryItem.create("mutant_beast_core", "异变兽核", 1, 1, 0.3, "legendary", "异变灵兽内丹，蕴含狂暴灵力"),
                0, 5
            )
            .gridItem(
                InventoryItem.create("baomai_scripture", "《爆脉流正法》", 1, 2, 1.0, "rare", "记载爆脉流修炼法门的残卷"),
                3, 2
            )
            .gridItem(
                InventoryItem.create("spirit_wood", "灵木", 1, 1, 2.0, "common", "蕴含微量灵气的木材"),
                3, 4
            )
            .gridItem(
                InventoryItem.create("zhenyuan_mine", "真元诡雷", 2, 1, 1.5, "uncommon", "以真元驱动的陷阱，触之即爆"),
                4, 5
            )
            .gridItem(
                InventoryItem.create("rat_tail", "噬元鼠尾", 1, 1, 0.4, "common", "噬元鼠的膨胀尾巴，可做炼器辅材"),
                3, 5
            )
            // Equipment
            .equip(EquipSlotType.HEAD,
                InventoryItem.create("ash_spider_silk", "灰蛛丝头巾", 1, 1, 0.3, "uncommon", "拟态灰烬蛛丝编织，轻薄坚韧"))
            .equip(EquipSlotType.CHEST,
                InventoryItem.create("fake_spirit_hide", "伪灵兽皮甲", 2, 2, 4.0, "rare", "以伪灵皮缝制的胸甲，可抵御低阶法术"))
            .equip(EquipSlotType.MAIN_HAND,
                InventoryItem.create("bone_spike", "骨刺短剑", 1, 2, 1.5, "uncommon", "三根骨刺捆绑而成的近战武器"))
            .equip(EquipSlotType.OFF_HAND,
                InventoryItem.create("decoy_stake", "替身木桩", 1, 1, 2.0, "rare", "欺天阵法器，可替主人挡一次致命伤"))
            // Hotbar
            .hotbar(0, InventoryItem.create("ningmai_powder", "凝脉散", 1, 1, 0.3, "uncommon", "外敷经脉，缓解走火入魔"))
            .hotbar(1, InventoryItem.create("huiyuan_pill_forbidden", "回元丹(禁药)", 1, 1, 0.2, "legendary", "禁药版回元丹，极速回复真元但有反噬"))
            .hotbar(2, InventoryItem.create("fengling_bone_coin", "封灵骨币", 1, 1, 0.1, "rare", "刻有封灵阵法的骨质钱币"))
            // Stats
            .weight(12.4, 50.0)
            .spiritStones(342)
            .cultivation("炼气三层", 78.0, 100.0, 0.45)
            .build();
    }
}
