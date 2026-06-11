package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-shield-block-v1 §P4：canPlaceIntoQuickUse 盾排除饱和测试。
 *
 * <p>覆盖：
 * <ul>
 *   <li>wooden_shield / bone_shield 不能放入 QuickUse 槽（正例）</li>
 *   <li>消耗品（guyuan_pill）仍可放入 QuickUse 槽（负例，确保未误伤）</li>
 *   <li>1×1 盾也被拒（isSingleCell=true 时唯一拦截点是 !isShield）</li>
 *   <li>空 item / null 被安全拒绝（边界）</li>
 * </ul>
 */
class InventoryEquipRulesQuickUseTest {

    // ── 盾牌被拒 ──────────────────────────────────────────────────────────

    @Test
    void woodenShield_cannotPlaceIntoQuickUse() {
        InventoryItem shield = item(7001L, "wooden_shield", 1, 2);
        assertFalse(
            InventoryEquipRules.canPlaceIntoQuickUse(shield),
            "期望 canPlaceIntoQuickUse(wooden_shield)=false（盾必须走 off_hand 装备路径，不应进 QuickUse 槽），"
                + "实际返回 true——canPlaceIntoQuickUse 未排除 isShield"
        );
    }

    @Test
    void boneShield_cannotPlaceIntoQuickUse() {
        InventoryItem shield = item(7002L, "bone_shield", 1, 2);
        assertFalse(
            InventoryEquipRules.canPlaceIntoQuickUse(shield),
            "期望 canPlaceIntoQuickUse(bone_shield)=false，实际返回 true"
        );
    }

    @Test
    void singleCellWoodenShield_cannotPlaceIntoQuickUse() {
        // 用 1×1 盾使 isSingleCell 短路失效，唯一拦截必须是 !isShield
        InventoryItem shield = item(7003L, "wooden_shield", 1, 1);
        assertFalse(
            InventoryEquipRules.canPlaceIntoQuickUse(shield),
            "期望 canPlaceIntoQuickUse(1×1 wooden_shield)=false：isSingleCell=true 时唯一拦截点是 !isShield；"
                + "若此断言变绿说明 canPlaceIntoQuickUse 丢了 !isShield 子句"
        );
    }

    // ── 非盾不受影响 ───────────────────────────────────────────────────────

    @Test
    void consumable_canPlaceIntoQuickUse() {
        InventoryItem pill = item(3001L, "guyuan_pill", 1, 1);
        assertTrue(
            InventoryEquipRules.canPlaceIntoQuickUse(pill),
            "期望 canPlaceIntoQuickUse(guyuan_pill)=true（消耗品应可放入 QuickUse 槽），"
                + "实际返回 false——isShield 排除意外影响了消耗品"
        );
    }

    @Test
    void consumable_herbMedicine_canPlaceIntoQuickUse() {
        InventoryItem herb = item(3002L, "spirit_herb", 1, 1);
        assertTrue(
            InventoryEquipRules.canPlaceIntoQuickUse(herb),
            "期望 canPlaceIntoQuickUse(spirit_herb)=true，实际返回 false"
        );
    }

    // ── 边界：空/null ───────────────────────────────────────────────────────

    @Test
    void nullItem_cannotPlaceIntoQuickUse() {
        assertFalse(
            InventoryEquipRules.canPlaceIntoQuickUse(null),
            "canPlaceIntoQuickUse(null) 应返回 false（安全降级），实际返回 true"
        );
    }

    @Test
    void emptyItem_withEmptyItemId_isNotBlockedByShieldGuard() {
        // itemId="" 不在 SHIELD_TEMPLATE_IDS，isSingleCell=true，所以 canPlaceIntoQuickUse=true。
        // 本 case 验证：空 itemId 走了通路，盾排除逻辑不影响非盾路径。
        // 真实调用场景不会出现空 itemId（UI 层先过滤），此 case 作为边界文档存在。
        InventoryItem empty = InventoryItem.create("", "", 1, 1, 0.5, "common", "");
        assertTrue(
            InventoryEquipRules.canPlaceIntoQuickUse(empty),
            "canPlaceIntoQuickUse(itemId='') 应返回 true（空 itemId 不在 SHIELD_TEMPLATE_IDS，不被排除），"
                + "实际返回 false——说明 isShield 意外影响了非盾路径"
        );
    }

    // ── helpers ────────────────────────────────────────────────────────────

    private static InventoryItem item(long instanceId, String itemId, int gridWidth, int gridHeight) {
        return InventoryItem.createFull(
            instanceId, itemId, itemId, gridWidth, gridHeight,
            1.0, "common", "", 1, 1.0, 1.0
        );
    }
}
