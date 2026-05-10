package com.bong.client.craft;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import org.junit.jupiter.api.Test;

import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CraftUxViewModelTest {

    @Test
    void screenHeightMatchesAlchemyTabHeight() {
        assertEquals(640, CraftScreenLayout.PANEL_W);
        assertEquals(340, CraftScreenLayout.PANEL_H);
        assertTrue(CraftScreenLayout.matchesAlchemyTabHeight());
        assertEquals(44, CraftScreenLayout.MATERIAL_SLOT_SIZE);
        assertEquals(32, CraftScreenLayout.ACTION_BAR_H);
    }

    @Test
    void materialStatesTrackSufficientAndMissingCounts() {
        CraftRecipe recipe = recipe("armor", CraftCategory.TOOL, true,
            List.of(
                new CraftRecipe.MaterialEntry("iron_ore", 5),
                new CraftRecipe.MaterialEntry("bone_coin", 3)
            ),
            0.0
        );
        InventoryModel inventory = InventoryModel.builder()
            .gridItem(stack("iron_ore", 7), 0, 0)
            .hotbar(0, stack("bone_coin", 1))
            .build();

        List<CraftMaterialState> states = CraftInventoryCounter.materialStates(recipe, inventory);
        assertTrue(states.get(0).sufficient());
        assertFalse(states.get(1).sufficient());
        assertEquals(2, states.get(1).missing());
    }

    @Test
    void materialStatesMultiplyNeedsBySelectedQuantity() {
        CraftRecipe recipe = recipe("armor", CraftCategory.TOOL, true,
            List.of(new CraftRecipe.MaterialEntry("iron_ore", 5)),
            0.0
        );
        InventoryModel inventory = InventoryModel.builder()
            .gridItem(stack("iron_ore", 7), 0, 0)
            .build();

        List<CraftMaterialState> states = CraftInventoryCounter.materialStates(recipe, inventory, 2);
        assertEquals(10, states.get(0).need());
        assertFalse(states.get(0).sufficient());
        assertEquals(3, states.get(0).missing());
    }

    @Test
    void maxCraftableUsesMaterialsAndQi() {
        CraftRecipe recipe = recipe("knife", CraftCategory.TOOL, true,
            List.of(new CraftRecipe.MaterialEntry("iron_ingot", 2)),
            4.0
        );
        InventoryModel inventory = InventoryModel.builder()
            .gridItem(stack("iron_ingot", 9), 0, 0)
            .cultivation("Awaken", 10.0, 20.0, 0.0)
            .build();

        assertEquals(2, CraftInventoryCounter.maxCraftable(recipe, inventory),
            "材料可做 4 个，但 qi 只够 2 个");
    }

    @Test
    void equippedItemsDoNotCountAsCraftMaterials() {
        CraftRecipe recipe = recipe("tool", CraftCategory.TOOL, true,
            List.of(new CraftRecipe.MaterialEntry("iron_ingot", 2)),
            0.0
        );
        InventoryModel inventory = InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, stack("iron_ingot", 2))
            .build();

        assertEquals(0, CraftInventoryCounter.countTemplate(inventory, "iron_ingot"));
        assertEquals(0, CraftInventoryCounter.maxCraftable(recipe, inventory));
        assertTrue(CraftInventoryCounter.isEquippedMaterial(inventory, "iron_ingot"));
    }

    @Test
    void filterSearchesNameMaterialAndPinsFavorites() {
        CraftRecipe armor = recipe("armor", CraftCategory.TOOL, true,
            List.of(new CraftRecipe.MaterialEntry("iron_ore", 5)),
            0.0
        );
        CraftRecipe potion = recipe("potion", CraftCategory.DUGU_POTION, true,
            List.of(new CraftRecipe.MaterialEntry("bitter_herb", 2)),
            0.0
        );
        List<CraftRecipe> result = CraftRecipeFilter.filter(
            List.of(armor, potion),
            null,
            "herb",
            new LinkedHashSet<>(Set.of("potion"))
        );
        assertEquals(List.of(potion), result);

        List<CraftRecipe> pinned = CraftRecipeFilter.filter(
            List.of(armor, potion),
            null,
            "",
            new LinkedHashSet<>(Set.of("potion"))
        );
        assertEquals("potion", pinned.get(0).id());
    }

    @Test
    void lockedRecipeShowsQuestionMarksAndHint() {
        CraftRecipe locked = recipe("secret", CraftCategory.MISC, false, List.of(), 0.0);
        assertEquals("???", CraftRecipeFilter.displayName(locked));
        assertEquals("引气 / 残卷 / 师承", CraftRecipeFilter.unlockHint(locked));
    }

    private static CraftRecipe recipe(
        String id,
        CraftCategory category,
        boolean unlocked,
        List<CraftRecipe.MaterialEntry> materials,
        double qiCost
    ) {
        return new CraftRecipe(
            id,
            category,
            "配方 " + id,
            materials,
            qiCost,
            60L,
            id + "_output",
            1,
            CraftRecipe.Requirements.NONE,
            unlocked
        );
    }

    private static InventoryItem stack(String itemId, int count) {
        return InventoryItem.createFull(
            1L,
            itemId,
            itemId,
            1,
            1,
            1.0,
            "common",
            "",
            count,
            1.0,
            1.0
        );
    }
}
