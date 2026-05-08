package com.bong.client.craft;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CraftStoreTest {

    @BeforeEach
    void setUp() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
    }

    @AfterEach
    void tearDown() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
    }

    private static CraftRecipe sampleRecipe(String id, CraftCategory category, boolean unlocked) {
        return new CraftRecipe(
            id,
            category,
            "示例 " + id,
            List.of(new CraftRecipe.MaterialEntry("herb", 2)),
            5.0,
            120L,
            "output_" + id,
            1,
            CraftRecipe.Requirements.NONE,
            unlocked
        );
    }

    @Test
    void replaceRecipesNotifiesListenerWithFullSnapshot() {
        AtomicInteger count = new AtomicInteger();
        CraftStore.addRecipeListener(list -> {
            assertEquals(2, list.size());
            count.incrementAndGet();
        });
        CraftStore.replaceRecipes(List.of(
            sampleRecipe("a", CraftCategory.TOOL, true),
            sampleRecipe("b", CraftCategory.DUGU_POTION, false)
        ));
        assertEquals(1, count.get());
        assertEquals(2, CraftStore.recipes().size());
    }

    @Test
    void recipeLookupByIdReturnsExpectedEntry() {
        CraftStore.replaceRecipes(List.of(sampleRecipe("alpha", CraftCategory.MISC, true)));
        assertTrue(CraftStore.recipe("alpha").isPresent());
        assertEquals("示例 alpha", CraftStore.recipe("alpha").get().displayName());
        assertTrue(CraftStore.recipe("missing").isEmpty());
    }

    @Test
    void markRecipeUnlockedFlipsUnlockedFlag() {
        CraftStore.replaceRecipes(List.of(sampleRecipe("locked", CraftCategory.TOOL, false)));
        assertFalse(CraftStore.recipe("locked").orElseThrow().unlocked());
        CraftStore.markRecipeUnlocked("locked");
        assertTrue(CraftStore.recipe("locked").orElseThrow().unlocked());
    }

    @Test
    void markRecipeUnlockedNoOpsWhenAlreadyUnlocked() {
        CraftStore.replaceRecipes(List.of(sampleRecipe("ready", CraftCategory.TOOL, true)));
        List<CraftRecipe> before = CraftStore.recipes();
        CraftStore.markRecipeUnlocked("ready");
        assertSame(before, CraftStore.recipes(),
            "already-unlocked recipe should noop replaceRecipes (引用相同表示未触发 replace)");
    }

    @Test
    void replaceSessionNotifiesOnTransitionAndDedupes() {
        AtomicInteger count = new AtomicInteger();
        CraftStore.addSessionListener(state -> count.incrementAndGet());
        // 初始 IDLE → 同状态再写一次不应回调
        CraftStore.replaceSession(CraftSessionStateView.IDLE);
        assertEquals(0, count.get(), "等价 session 不应触发 listener");
        CraftStore.replaceSession(new CraftSessionStateView(true, "x", 10, 100));
        assertEquals(1, count.get());
        // 同 active 同 elapsed 不变化 → noop
        CraftStore.replaceSession(new CraftSessionStateView(true, "x", 10, 100));
        assertEquals(1, count.get());
        // elapsed 变化触发
        CraftStore.replaceSession(new CraftSessionStateView(true, "x", 30, 100));
        assertEquals(2, count.get());
    }

    @Test
    void recordUnlockUpdatesRecipeFlagAndNotifiesUnlockListener() {
        CraftStore.replaceRecipes(List.of(sampleRecipe("alpha", CraftCategory.TOOL, false)));
        AtomicInteger unlockCount = new AtomicInteger();
        CraftStore.addUnlockListener(event -> unlockCount.incrementAndGet());
        CraftStore.recordUnlock(new CraftStore.RecipeUnlockedEvent(
            "alpha",
            new CraftStore.RecipeUnlockedEvent.Scroll("scroll_alpha"),
            42L
        ));
        assertTrue(CraftStore.recipe("alpha").orElseThrow().unlocked());
        assertEquals(1, unlockCount.get());
        assertTrue(CraftStore.lastUnlocked().isPresent());
    }

    @Test
    void recordOutcomeStoresLastOutcomeAndNotifies() {
        AtomicInteger count = new AtomicInteger();
        CraftStore.addOutcomeListener(event -> count.incrementAndGet());
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "alpha", "alpha_output", 3, 1000L));
        assertTrue(CraftStore.lastOutcome().isPresent());
        assertEquals(CraftStore.CraftOutcomeEvent.Kind.COMPLETED,
            CraftStore.lastOutcome().get().kind());
        assertEquals(1, count.get());
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.failed(
            "alpha", "player_cancelled", 2, 0.0));
        assertEquals(CraftStore.CraftOutcomeEvent.Kind.FAILED,
            CraftStore.lastOutcome().get().kind());
        assertEquals(2, count.get());
    }

    @Test
    void groupedByCategoryPreservesInsertionOrder() {
        CraftStore.replaceRecipes(List.of(
            sampleRecipe("a", CraftCategory.TOOL, true),
            sampleRecipe("b", CraftCategory.DUGU_POTION, false),
            sampleRecipe("c", CraftCategory.TOOL, true)
        ));
        var grouped = CraftStore.recipesGroupedByCategory();
        assertEquals(2, grouped.get(CraftCategory.TOOL).size());
        assertEquals(1, grouped.get(CraftCategory.DUGU_POTION).size());
    }

    @Test
    void clearResetsEverythingIncludingLastOutcomeAndUnlock() {
        CraftStore.replaceRecipes(List.of(sampleRecipe("x", CraftCategory.TOOL, true)));
        CraftStore.replaceSession(new CraftSessionStateView(true, "x", 5, 10));
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed("x", "x", 1, 1));
        CraftStore.recordUnlock(new CraftStore.RecipeUnlockedEvent(
            "x", new CraftStore.RecipeUnlockedEvent.Scroll("scroll_x"), 1));
        CraftStore.clear();
        assertEquals(0, CraftStore.recipes().size());
        assertFalse(CraftStore.sessionState().active());
        assertFalse(CraftStore.lastOutcome().isPresent(),
            "clear() 必须把 lastOutcome 重置为空，避免 UI 弹老的出炉提示");
        assertFalse(CraftStore.lastUnlocked().isPresent(),
            "clear() 必须把 lastUnlocked 重置为空");
    }
}
