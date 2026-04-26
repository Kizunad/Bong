package com.bong.client.inventory.model;

/**
 * 模拟体表状态 — 刚经历一场战斗的引气修士。
 * 右前臂断了（爆脉流后遗症），左大腿骨折，多处擦伤割裂。
 */
public final class MockPhysicalData {
    private MockPhysicalData() {}

    public static PhysicalBody create() {
        return PhysicalBody.builder()
            // 头部擦伤
            .wound(BodyPart.HEAD, WoundLevel.ABRASION, 0.0, 0.3, false)
            // 胸腔割裂（被人一掌拍的）
            .wound(BodyPart.CHEST, WoundLevel.LACERATION, 0.15, 0.0, false)
            // 右上臂割裂
            .wound(BodyPart.RIGHT_UPPER_ARM, WoundLevel.LACERATION, 0.1, 0.0, false)
            // 右前臂断了！（爆脉流反噬）
            .wound(BodyPart.RIGHT_FOREARM, WoundLevel.SEVERED, 0.3, 0.0, false)
            // 右手随前臂断了
            .wound(BodyPart.RIGHT_HAND, WoundLevel.SEVERED, 0.0, 0.0, false)
            // 左手淤伤
            .wound(BodyPart.LEFT_HAND, WoundLevel.BRUISE, 0.0, 0.5, false)
            // 左大腿骨折（已上夹板）
            .wound(BodyPart.LEFT_THIGH, WoundLevel.FRACTURE, 0.0, 0.2, true)
            // 左小腿擦伤
            .wound(BodyPart.LEFT_CALF, WoundLevel.ABRASION, 0.0, 0.6, false)
            // 已用物品
            .appliedItem(BodyPart.CHEST,
                InventoryItem.create("ash_spider_silk", "灰蛛丝绷带", 1, 1, 0.1, "uncommon", "灰烬蛛丝编织，止血效果极佳"))
            .appliedItem(BodyPart.RIGHT_UPPER_ARM,
                InventoryItem.create("ningmai_powder", "凝脉散", 1, 1, 0.3, "uncommon", "外敷经脉，缓解走火入魔"))
            .build();
    }
}
