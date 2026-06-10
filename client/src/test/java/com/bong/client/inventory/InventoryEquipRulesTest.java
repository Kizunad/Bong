package com.bong.client.inventory;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import java.util.EnumMap;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class InventoryEquipRulesTest {

    @Test
    void swordCanEquipMainHandButNotOffHandOrHotbar() {
        InventoryItem sword = item(1001L, "iron_sword", 1, 2);

        assertTrue(InventoryEquipRules.canEquip(sword, EquipSlotType.MAIN_HAND, null, equipped()));
        assertFalse(InventoryEquipRules.canEquip(sword, EquipSlotType.OFF_HAND, null, equipped()));
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(item(1002L, "bone_dagger", 1, 1)));
    }

    @Test
    void daggerCanFallbackIntoOffHandWhenMainHandOccupied() {
        InventoryItem dagger = item(1001L, "bone_dagger", 1, 1);
        EnumMap<EquipSlotType, InventoryItem> equipped = equipped();
        equipped.put(EquipSlotType.MAIN_HAND, item(2002L, "iron_sword", 1, 2));

        assertEquals(
            EquipSlotType.OFF_HAND,
            InventoryEquipRules.preferredWeaponQuickEquipSlot(dagger, equipped, slot -> true)
        );
    }

    @Test
    void quickEquipDoesNotRouteWeaponsIntoArmorSlots() {
        InventoryItem sword = item(1001L, "iron_sword", 1, 2);

        assertEquals(
            EquipSlotType.MAIN_HAND,
            InventoryEquipRules.preferredWeaponQuickEquipSlot(sword, equipped(), slot -> true)
        );
    }

    @Test
    void twoHandWeaponIsRejectedWhileMainHandOccupied() {
        InventoryItem staff = item(1001L, "wooden_staff", 1, 3);
        EnumMap<EquipSlotType, InventoryItem> equipped = equipped();
        equipped.put(EquipSlotType.MAIN_HAND, item(2002L, "iron_sword", 1, 2));

        assertFalse(InventoryEquipRules.canEquip(staff, EquipSlotType.TWO_HAND, null, equipped));
    }

    @Test
    void movingFromTwoHandToMainHandMirrorsServerException() {
        InventoryItem staff = item(1001L, "wooden_staff", 1, 3);
        EnumMap<EquipSlotType, InventoryItem> equipped = equipped();
        equipped.put(EquipSlotType.TWO_HAND, staff);

        assertTrue(InventoryEquipRules.canEquip(
            staff,
            EquipSlotType.MAIN_HAND,
            EquipSlotType.TWO_HAND,
            equipped
        ));
    }

    @Test
    void toolCanEquipMainHandButNotHotbarOrArmor() {
        InventoryItem tool = item(5005L, "dun_qi_jia", 1, 1);

        assertTrue(InventoryEquipRules.canEquip(tool, EquipSlotType.MAIN_HAND, null, equipped()));
        assertTrue(InventoryEquipRules.isTool(tool));
        assertFalse(InventoryEquipRules.canEquip(tool, EquipSlotType.CHEST, null, equipped()));
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(tool));
    }

    @Test
    void quickEquipRoutesToolToMainHand() {
        InventoryItem tool = item(5005L, "dun_qi_jia", 1, 1);

        assertEquals(
            EquipSlotType.MAIN_HAND,
            InventoryEquipRules.preferredWeaponQuickEquipSlot(tool, equipped(), slot -> true)
        );
    }

    @Test
    void toolCannotEquipMainHandWhileTwoHandOccupied() {
        InventoryItem staff = item(1001L, "wooden_staff", 1, 3);
        InventoryItem tool = item(5005L, "dun_qi_jia", 1, 1);
        EnumMap<EquipSlotType, InventoryItem> equipped = equipped();
        equipped.put(EquipSlotType.TWO_HAND, staff);

        assertFalse(InventoryEquipRules.canEquip(tool, EquipSlotType.MAIN_HAND, null, equipped));
    }

    @Test
    void consumablesStayHotbarCompatible() {
        assertTrue(InventoryEquipRules.canPlaceIntoHotbar(item(3003L, "guyuan_pill", 1, 1)));
    }

    @Test
    void treasureCanEquipOffHandButNotHotbar() {
        InventoryItem treasure = item(4004L, "starter_talisman", 1, 1);

        assertTrue(InventoryEquipRules.canEquip(treasure, EquipSlotType.OFF_HAND, null, equipped()));
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(treasure));
    }

    @Test
    void mundaneArmorEquipsOnlyMatchingArmorSlot() {
        InventoryItem chestplate = item(6006L, "armor_bone_chestplate", 2, 2);

        assertTrue(InventoryEquipRules.canEquip(chestplate, EquipSlotType.CHEST, null, equipped()));
        assertFalse(InventoryEquipRules.canEquip(chestplate, EquipSlotType.HEAD, null, equipped()));
        assertTrue(InventoryEquipRules.isArmor(chestplate));
    }

    @Test
    void brokenArmorCannotEquipAndArmorCannotUseHotbar() {
        InventoryItem brokenBoots = InventoryItem.createFull(
            6007L,
            "armor_bone_boots",
            "骨甲靴",
            1,
            1,
            1.0,
            "common",
            "",
            1,
            1.0,
            0.0
        );

        assertFalse(InventoryEquipRules.canEquip(brokenBoots, EquipSlotType.FEET, null, equipped()));
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(brokenBoots));
    }

    // ── plan-shield-block-v1 P0 — client InventoryEquipRules 盾牌路由饱和测试 ──

    /** wooden_shield 装 off_hand 应放行（isShield 分支）。 */
    @Test
    void woodenShieldCanEquipOffHand() {
        InventoryItem shield = item(7001L, "wooden_shield", 1, 2);

        assertTrue(
            InventoryEquipRules.canEquip(shield, EquipSlotType.OFF_HAND, null, equipped()),
            "期望 wooden_shield 可装入 OFF_HAND（plan-shield-block-v1 P0 isShield 分支），" +
            "实际 canEquip 返回 false——检查 SHIELD_TEMPLATE_IDS 是否包含 wooden_shield"
        );
    }

    /** bone_shield 装 off_hand 也应放行。 */
    @Test
    void boneShieldCanEquipOffHand() {
        InventoryItem shield = item(7002L, "bone_shield", 1, 2);

        assertTrue(
            InventoryEquipRules.canEquip(shield, EquipSlotType.OFF_HAND, null, equipped()),
            "期望 bone_shield 可装入 OFF_HAND（plan-shield-block-v1 P0），" +
            "实际 canEquip 返回 false"
        );
    }

    /** two_hand 占用时盾牌装 off_hand 应被拒绝（边界）。 */
    @Test
    void shieldCannotEquipOffHandWhenTwoHandOccupied() {
        InventoryItem shield = item(7001L, "wooden_shield", 1, 2);
        EnumMap<EquipSlotType, InventoryItem> equipped = equipped();
        equipped.put(EquipSlotType.TWO_HAND, item(9999L, "wooden_staff", 1, 3));

        assertFalse(
            InventoryEquipRules.canEquip(shield, EquipSlotType.OFF_HAND, null, equipped),
            "期望 two_hand 占用时盾牌装 OFF_HAND 被拒绝（边界），" +
            "实际 canEquip 返回 true——检查 two_hand occupied 逻辑"
        );
    }

    /** 非盾非 treasure 非 dagger 物品装 off_hand 仍拒绝（回归保护）。 */
    @Test
    void swordIsStillRejectedFromOffHand() {
        InventoryItem sword = item(1001L, "iron_sword", 1, 2);

        assertFalse(
            InventoryEquipRules.canEquip(sword, EquipSlotType.OFF_HAND, null, equipped()),
            "期望 iron_sword 装 OFF_HAND 仍被拒绝（Shield 分支不影响 Sword），" +
            "实际 canEquip 返回 true——Shield 分支意外放行了 sword"
        );
    }

    /** isShield 公开方法对 wooden_shield 返回 true。 */
    @Test
    void isShieldReturnsTrueForKnownShields() {
        assertTrue(
            InventoryEquipRules.isShield(item(7001L, "wooden_shield", 1, 2)),
            "期望 isShield(wooden_shield) = true，实际返回 false"
        );
        assertTrue(
            InventoryEquipRules.isShield(item(7002L, "bone_shield", 1, 2)),
            "期望 isShield(bone_shield) = true，实际返回 false"
        );
    }

    /** isShield 对非盾物品返回 false。 */
    @Test
    void isShieldReturnsFalseForNonShields() {
        assertFalse(
            InventoryEquipRules.isShield(item(1001L, "iron_sword", 1, 2)),
            "期望 isShield(iron_sword) = false，实际返回 true"
        );
        assertFalse(
            InventoryEquipRules.isShield(item(4004L, "starter_talisman", 1, 1)),
            "期望 isShield(starter_talisman) = false（treasure 不是盾），实际返回 true"
        );
    }

    private static EnumMap<EquipSlotType, InventoryItem> equipped() {
        return new EnumMap<>(EquipSlotType.class);
    }

    private static InventoryItem item(long instanceId, String itemId, int gridWidth, int gridHeight) {
        return InventoryItem.createFull(
            instanceId,
            itemId,
            itemId,
            gridWidth,
            gridHeight,
            1.0,
            "common",
            "",
            1,
            1.0,
            1.0
        );
    }
}
