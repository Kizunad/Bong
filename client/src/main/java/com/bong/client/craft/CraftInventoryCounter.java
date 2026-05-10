package com.bong.client.craft;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;

import java.util.ArrayList;
import java.util.List;

/** plan-craft-ux-v1 — 从权威背包快照计算材料状态。 */
public final class CraftInventoryCounter {
    private CraftInventoryCounter() {}

    public static int countTemplate(InventoryModel inventory, String templateId) {
        if (inventory == null || templateId == null || templateId.isBlank()) {
            return 0;
        }
        long total = 0;
        for (InventoryModel.GridEntry entry : inventory.gridItems()) {
            total += countIfMatches(entry.item(), templateId);
        }
        for (InventoryItem item : inventory.hotbar()) {
            total += countIfMatches(item, templateId);
        }
        return total > Integer.MAX_VALUE ? Integer.MAX_VALUE : (int) total;
    }

    public static List<CraftMaterialState> materialStates(CraftRecipe recipe, InventoryModel inventory) {
        return materialStates(recipe, inventory, 1);
    }

    public static List<CraftMaterialState> materialStates(CraftRecipe recipe, InventoryModel inventory, int quantity) {
        if (recipe == null) {
            return List.of();
        }
        int multiplier = Math.max(1, quantity);
        List<CraftMaterialState> states = new ArrayList<>(recipe.materials().size());
        for (CraftRecipe.MaterialEntry material : recipe.materials()) {
            states.add(new CraftMaterialState(
                material.templateId(),
                saturatingMultiply(Math.max(0, material.count()), multiplier),
                countTemplate(inventory, material.templateId())
            ));
        }
        return states;
    }

    public static boolean hasAllMaterials(CraftRecipe recipe, InventoryModel inventory) {
        if (recipe == null || !recipe.unlocked()) {
            return false;
        }
        if (inventory == null) {
            inventory = InventoryModel.empty();
        }
        for (CraftMaterialState state : materialStates(recipe, inventory)) {
            if (!state.sufficient()) {
                return false;
            }
        }
        return recipe.qiCost() <= 0.0 || inventory.qiCurrent() >= recipe.qiCost();
    }

    public static int maxCraftable(CraftRecipe recipe, InventoryModel inventory) {
        if (recipe == null || !recipe.unlocked()) {
            return 0;
        }
        if (inventory == null) {
            inventory = InventoryModel.empty();
        }
        int max = Integer.MAX_VALUE;
        for (CraftRecipe.MaterialEntry material : recipe.materials()) {
            int need = Math.max(0, material.count());
            if (need == 0) {
                continue;
            }
            max = Math.min(max, countTemplate(inventory, material.templateId()) / need);
        }
        if (recipe.qiCost() > 0.0) {
            max = Math.min(max, (int) Math.floor(inventory.qiCurrent() / recipe.qiCost()));
        }
        if (max == Integer.MAX_VALUE) {
            max = 64;
        }
        return Math.max(0, Math.min(64, max));
    }

    public static boolean isEquippedMaterial(InventoryModel inventory, String templateId) {
        if (inventory == null || templateId == null) {
            return false;
        }
        for (EquipSlotType ignored : inventory.equipped().keySet()) {
            InventoryItem item = inventory.equipped().get(ignored);
            if (countIfMatches(item, templateId) > 0) {
                return true;
            }
        }
        return false;
    }

    private static int countIfMatches(InventoryItem item, String templateId) {
        if (item == null || item.isEmpty() || !templateId.equals(item.itemId())) {
            return 0;
        }
        return Math.max(0, item.stackCount());
    }

    private static int saturatingMultiply(int value, int multiplier) {
        long result = (long) value * (long) multiplier;
        return result > Integer.MAX_VALUE ? Integer.MAX_VALUE : (int) result;
    }
}
