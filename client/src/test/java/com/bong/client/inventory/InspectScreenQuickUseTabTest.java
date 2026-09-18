package com.bong.client.inventory;

import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class InspectScreenQuickUseTabTest {
    @AfterEach
    void resetCraftStore() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
    }

    @Test
    void craftEntryStillOpensDedicatedScreen() {
        assertTrue(
            InspectScreen.opensCraftScreenForTabForTests(3),
            "合并修习后，手搓入口仍应打开制作界面"
        );
    }

    @Test
    void craftEntryStatusShowsKnownRecipeCountBoundaries() {
        CraftStore.replaceRecipes(List.of());
        assertEquals(
            "已知配方 0",
            InspectScreen.craftStatusLineForTests(),
            "expected zero recipe status because CraftStore has no recipes, actual "
                + InspectScreen.craftStatusLineForTests()
        );

        CraftStore.replaceRecipes(List.of(recipe("r0"), recipe("r1")));
        assertEquals(
            "已知配方 2",
            InspectScreen.craftStatusLineForTests(),
            "expected known recipe count to follow CraftStore.recipes().size(), actual "
                + InspectScreen.craftStatusLineForTests()
        );

        CraftStore.replaceRecipes(java.util.stream.IntStream.range(0, 64)
            .mapToObj(i -> recipe("bulk_" + i))
            .toList());
        assertEquals(
            "已知配方 64",
            InspectScreen.craftStatusLineForTests(),
            "expected large recipe count to render without clamping because status mirrors CraftStore size, actual "
                + InspectScreen.craftStatusLineForTests()
        );
    }

    @Test
    void craftEntryStatusPrioritizesActiveSession() {
        CraftStore.replaceRecipes(List.of(recipe("r0")));
        CraftStore.replaceSession(new CraftSessionStateView(true, "r0", 20L, 100L));

        assertEquals(
            "当前任务进行中",
            InspectScreen.craftStatusLineForTests(),
            "expected active session status because an in-progress craft should override recipe count, actual "
                + InspectScreen.craftStatusLineForTests()
        );
    }

    private static CraftRecipe recipe(String id) {
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
