package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.skill.SkillSetSnapshot;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CraftScreenViewModelTest {
    @Test
    void copiesRecipeListAndSupportsExactLookup() {
        CraftRecipe recipe = recipe("rough_handle");
        List<CraftRecipe> mutable = new ArrayList<>(List.of(recipe));
        CraftScreenViewModel model = model(3L, mutable);

        mutable.clear();

        assertEquals(List.of(recipe), model.recipes(),
            "ViewModel 必须复制 Store 列表，外部后续修改不能穿透到 UI");
        assertEquals(recipe, model.recipe("rough_handle").orElseThrow(),
            "recipe lookup 必须按完整 id 返回同一不可变 DTO");
        assertTrue(model.recipe("missing").isEmpty(), "未知 id 必须返回 empty");
        assertTrue(model.recipe(null).isEmpty(), "null id 必须 fail closed 为 empty");
        assertThrows(UnsupportedOperationException.class, () -> model.recipes().clear(),
            "ViewModel 暴露的 recipes 不得允许调用者修改");
    }

    @Test
    void rejectsInvalidIdentityAndNullStateParts() {
        assertThrows(IllegalArgumentException.class, () -> new CraftScreenViewModel(
            -1L,
            CraftScreenViewModel.Change.INITIAL,
            List.of(),
            InventoryModel.empty(),
            SkillSetSnapshot.empty(),
            CraftSessionStateView.IDLE,
            Optional.empty()
        ));
        assertThrows(NullPointerException.class, () -> new CraftScreenViewModel(
            0L,
            null,
            List.of(),
            InventoryModel.empty(),
            SkillSetSnapshot.empty(),
            CraftSessionStateView.IDLE,
            Optional.empty()
        ));
    }

    private static CraftScreenViewModel model(long revision, List<CraftRecipe> recipes) {
        return new CraftScreenViewModel(
            revision,
            CraftScreenViewModel.Change.INITIAL,
            recipes,
            InventoryModel.empty(),
            SkillSetSnapshot.empty(),
            CraftSessionStateView.IDLE,
            Optional.empty()
        );
    }

    static CraftRecipe recipe(String id) {
        return new CraftRecipe(
            id,
            CraftCategory.TOOL,
            "配方 " + id,
            List.of(),
            0.0,
            20L,
            id + "_output",
            1,
            CraftRecipe.Requirements.NONE,
            true
        );
    }
}
