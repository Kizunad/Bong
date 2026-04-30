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
