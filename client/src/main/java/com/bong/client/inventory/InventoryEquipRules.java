package com.bong.client.inventory;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;

import java.util.Map;
import java.util.Set;
import java.util.function.Predicate;

final class InventoryEquipRules {
    private enum WeaponKind {
        SWORD,
        SABER,
        STAFF,
        FIST,
        SPEAR,
        DAGGER,
        BOW
    }

    private static final Map<String, WeaponKind> WEAPON_KIND_BY_ITEM_ID = Map.ofEntries(
        Map.entry("iron_sword", WeaponKind.SWORD),
        Map.entry("rusted_blade", WeaponKind.SWORD),
        Map.entry("spirit_sword", WeaponKind.SWORD),
        Map.entry("flying_sword_feixuan", WeaponKind.SWORD),
        Map.entry("bronze_saber", WeaponKind.SABER),
        Map.entry("wooden_staff", WeaponKind.STAFF),
        Map.entry("bone_dagger", WeaponKind.DAGGER),
        Map.entry("bone_spike", WeaponKind.DAGGER),
        Map.entry("poison_needle", WeaponKind.DAGGER),
        Map.entry("zhenyuan_mine", WeaponKind.DAGGER),
        Map.entry("hand_wrap", WeaponKind.FIST)
    );

    private static final Set<String> HOE_TEMPLATE_IDS = Set.of(
        "hoe_iron",
        "hoe_lingtie",
        "hoe_xuantie"
    );

    private static final Set<String> TOOL_TEMPLATE_IDS = Set.of(
        "cai_yao_dao",
        "bao_chu",
        "cao_lian",
        "dun_qi_jia",
        "gua_dao",
        "gu_hai_qian",
        "bing_jia_shou_tao"
    );

    private static final Set<String> TREASURE_TEMPLATE_IDS = Set.of(
        "starter_talisman",
        "broken_artifact"
    );

    private static final Set<String> FALSE_SKIN_TEMPLATE_IDS = Set.of(
        "tuike_false_skin_silk",
        "tuike_rotten_wood_armor"
    );

    private InventoryEquipRules() {
    }

    static boolean canEquip(
        InventoryItem item,
        EquipSlotType targetSlot,
        EquipSlotType sourceSlot,
        Map<EquipSlotType, InventoryItem> equipped
    ) {
        if (item == null || item.isEmpty() || targetSlot == null) return false;

        WeaponKind weaponKind = weaponKindOf(item.itemId());
        boolean hoe = isHoe(item.itemId());
        boolean tool = isTool(item.itemId());
        boolean fromTwoHand = sourceSlot == EquipSlotType.TWO_HAND;

        return switch (targetSlot) {
            case MAIN_HAND -> (weaponKind != null || hoe || tool)
                && (fromTwoHand || !isOccupied(equipped, EquipSlotType.TWO_HAND));
            case OFF_HAND -> ((weaponKind == WeaponKind.DAGGER || weaponKind == WeaponKind.FIST)
                || isTreasure(item))
                && (fromTwoHand || !isOccupied(equipped, EquipSlotType.TWO_HAND));
            case TWO_HAND -> (weaponKind == WeaponKind.SPEAR || weaponKind == WeaponKind.STAFF)
                && (fromTwoHand
                    || (!isOccupied(equipped, EquipSlotType.MAIN_HAND)
                    && !isOccupied(equipped, EquipSlotType.OFF_HAND)));
            case FALSE_SKIN -> isFalseSkin(item);
            case TREASURE_BELT_0, TREASURE_BELT_1, TREASURE_BELT_2, TREASURE_BELT_3 -> isTreasure(item);
            case HEAD, CHEST, LEGS, FEET -> weaponKind == null && !hoe && !tool && !isFalseSkin(item);
        };
    }

    static EquipSlotType preferredWeaponQuickEquipSlot(
        InventoryItem item,
        Map<EquipSlotType, InventoryItem> equipped,
        Predicate<EquipSlotType> usable
    ) {
        EquipSlotType[] order = {
            EquipSlotType.MAIN_HAND,
            EquipSlotType.OFF_HAND,
            EquipSlotType.TWO_HAND
        };
        for (EquipSlotType slot : order) {
            if (isOccupied(equipped, slot)) continue;
            if (!usable.test(slot)) continue;
            if (canEquip(item, slot, null, equipped)) return slot;
        }
        return null;
    }

    static boolean canPlaceIntoHotbar(InventoryItem item) {
        return isSingleCell(item) && !isWeapon(item) && !isTool(item) && !isTreasure(item);
    }

    static boolean canPlaceIntoQuickUse(InventoryItem item) {
        return isSingleCell(item);
    }

    static boolean isHoe(InventoryItem item) {
        return item != null && isHoe(item.itemId());
    }

    static boolean isWeapon(InventoryItem item) {
        return item != null && weaponKindOf(item.itemId()) != null;
    }

    static boolean isTreasure(InventoryItem item) {
        return item != null && isTreasure(item.itemId());
    }

    static boolean isTool(InventoryItem item) {
        return item != null && isTool(item.itemId());
    }

    private static boolean isSingleCell(InventoryItem item) {
        return item != null && item.gridWidth() == 1 && item.gridHeight() == 1;
    }

    private static boolean isOccupied(Map<EquipSlotType, InventoryItem> equipped, EquipSlotType slot) {
        if (equipped == null) return false;
        InventoryItem existing = equipped.get(slot);
        return existing != null && !existing.isEmpty();
    }

    private static boolean isHoe(String itemId) {
        return itemId != null && HOE_TEMPLATE_IDS.contains(itemId);
    }

    private static boolean isTreasure(String itemId) {
        return itemId != null && TREASURE_TEMPLATE_IDS.contains(itemId);
    }

    private static boolean isFalseSkin(InventoryItem item) {
        return item != null && FALSE_SKIN_TEMPLATE_IDS.contains(item.itemId());
    }

    private static boolean isTool(String itemId) {
        return itemId != null && TOOL_TEMPLATE_IDS.contains(itemId);
    }

    private static WeaponKind weaponKindOf(String itemId) {
        if (itemId == null || itemId.isBlank()) return null;
        WeaponKind explicit = WEAPON_KIND_BY_ITEM_ID.get(itemId);
        if (explicit != null) return explicit;

        String normalized = itemId.toLowerCase(java.util.Locale.ROOT);
        if (normalized.contains("staff")) return WeaponKind.STAFF;
        if (normalized.contains("spear")) return WeaponKind.SPEAR;
        if (normalized.contains("dagger") || normalized.contains("needle") || normalized.contains("spike")) {
            return WeaponKind.DAGGER;
        }
        if (normalized.contains("saber")) return WeaponKind.SABER;
        if (normalized.contains("sword") || normalized.contains("blade")) return WeaponKind.SWORD;
        if (normalized.contains("fist") || normalized.contains("wrap")) return WeaponKind.FIST;
        if (normalized.contains("bow")) return WeaponKind.BOW;
        return null;
    }
}
