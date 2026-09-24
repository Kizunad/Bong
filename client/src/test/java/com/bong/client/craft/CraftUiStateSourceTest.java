package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.ui.contract.UiSubscription;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CraftUiStateSourceTest {
    @BeforeEach
    void setUp() {
        resetStores();
    }

    @AfterEach
    void tearDown() {
        resetStores();
    }

    @Test
    void snapshotIsInitialAndSubscribeOnlyPublishesLaterChanges() {
        CraftUiStateSource source = CraftUiStateSource.production();
        List<CraftScreenViewModel> updates = new ArrayList<>();

        CraftScreenViewModel initial = source.snapshot();
        UiSubscription subscription = source.subscribe(updates::add);

        assertEquals(0L, initial.revision(), "新 source 的首帧 revision 必须从 0 开始");
        assertEquals(CraftScreenViewModel.Change.INITIAL, initial.change());
        assertTrue(updates.isEmpty(), "subscribe 不得重复立即推送首帧 snapshot");
        assertFalse(subscription.isClosed(), "生产 PUSH source 订阅后必须保持活跃");
    }

    @Test
    void allAuthoritativeChannelsProduceOrderedImmutableRevisions() {
        CraftUiStateSource source = CraftUiStateSource.production();
        List<CraftScreenViewModel> updates = new ArrayList<>();
        UiSubscription subscription = source.subscribe(updates::add);
        CraftRecipe recipe = CraftScreenViewModelTest.recipe("rough_handle");
        CraftSessionStateView session = new CraftSessionStateView(true, recipe.id(), 1L, 20L);
        CraftStore.CraftOutcomeEvent outcome = CraftStore.CraftOutcomeEvent.completed(
            recipe.id(), recipe.outputTemplate(), 1, 20L
        );
        InventoryModel inventory = InventoryModel.builder()
            .cultivation("Awaken", 8.0, 10.0, 1.0)
            .build();
        SkillSetSnapshot skills = SkillSetSnapshot.of(Map.of(
            SkillId.FORGING,
            new SkillSetSnapshot.Entry(2, 0L, 100L, 0L, 10, 0L, 0L)
        ));

        CraftStore.replaceRecipes(List.of(recipe));
        CraftStore.replaceSession(session);
        CraftStore.recordOutcome(outcome);
        InventoryStateStore.replace(inventory);
        SkillSetStore.replace(skills);

        assertEquals(
            List.of(
                CraftScreenViewModel.Change.RECIPES,
                CraftScreenViewModel.Change.SESSION,
                CraftScreenViewModel.Change.OUTCOME,
                CraftScreenViewModel.Change.INVENTORY,
                CraftScreenViewModel.Change.SKILLS
            ),
            updates.stream().map(CraftScreenViewModel::change).toList(),
            "五类 authoritative 变化必须保留发生顺序且各推一次"
        );
        assertEquals(List.of(1L, 2L, 3L, 4L, 5L),
            updates.stream().map(CraftScreenViewModel::revision).toList(),
            "每次可观察变化必须分配严格单调 revision");
        CraftScreenViewModel latest = updates.get(updates.size() - 1);
        assertEquals(List.of(recipe), latest.recipes());
        assertSame(session, latest.session(), "组合快照不得改写 session DTO");
        assertSame(inventory, latest.inventory(), "组合快照不得复制成语义不同的 inventory");
        assertSame(skills, latest.skills(), "组合快照不得改写 skill snapshot");
        assertEquals(CraftOutcomeView.from(outcome), latest.latestOutcome().orElseThrow());

        subscription.close();
    }

    @Test
    void repeatedEqualOutcomeStillGetsDistinctRevision() {
        CraftUiStateSource source = CraftUiStateSource.production();
        List<CraftScreenViewModel> updates = new ArrayList<>();
        UiSubscription subscription = source.subscribe(updates::add);
        CraftStore.CraftOutcomeEvent outcome = CraftStore.CraftOutcomeEvent.completed(
            "rough_handle", "rough_handle", 1, 20L
        );

        CraftStore.recordOutcome(outcome);
        CraftStore.recordOutcome(outcome);

        assertEquals(2, updates.size(), "两个独立 outcome 事件不得因 payload 相等而被折叠");
        assertEquals(List.of(1L, 2L), updates.stream().map(CraftScreenViewModel::revision).toList());
        assertTrue(updates.stream().allMatch(
            update -> update.change() == CraftScreenViewModel.Change.OUTCOME));
        subscription.close();
    }

    @Test
    void recipeUnlockUsesCanonicalRecipeChangeWithoutDuplicateUiEvent() {
        CraftRecipe locked = new CraftRecipe(
            "locked",
            CraftCategory.TOOL,
            "锁定配方",
            List.of(),
            0.0,
            20L,
            "locked_output",
            1,
            CraftRecipe.Requirements.NONE,
            false
        );
        CraftStore.replaceRecipes(List.of(locked));
        CraftUiStateSource source = CraftUiStateSource.production();
        List<CraftScreenViewModel> updates = new ArrayList<>();
        UiSubscription subscription = source.subscribe(updates::add);

        CraftStore.recordUnlock(new CraftStore.RecipeUnlockedEvent(
            locked.id(),
            new CraftStore.RecipeUnlockedEvent.Scroll("scroll_locked"),
            42L
        ));

        assertEquals(1, updates.size(),
            "recipe_unlocked 已经通过 recipe channel 更新，不应再制造第二次等价 UI 刷新");
        assertEquals(CraftScreenViewModel.Change.RECIPES, updates.get(0).change());
        assertTrue(updates.get(0).recipe(locked.id()).orElseThrow().unlocked());
        subscription.close();
    }

    @Test
    void closeIsIdempotentAndBlocksEveryLateStoreChannel() {
        CraftUiStateSource source = CraftUiStateSource.production();
        List<CraftScreenViewModel> updates = new ArrayList<>();
        UiSubscription subscription = source.subscribe(updates::add);

        subscription.close();
        subscription.close();
        CraftStore.replaceRecipes(List.of(CraftScreenViewModelTest.recipe("late")));
        CraftStore.replaceSession(new CraftSessionStateView(true, "late", 1L, 20L));
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.failed("late", "cancelled", 0, 0.0));
        InventoryStateStore.replace(InventoryModel.builder().build());
        SkillSetStore.replace(SkillSetSnapshot.empty());

        assertTrue(subscription.isClosed());
        assertTrue(updates.isEmpty(), "关闭后的任一 Store 变化都不得穿透到旧屏幕");
    }

    @Test
    void failingUiListenerCannotBreakStoreWriteOrFollowingStoreListener() {
        CraftUiStateSource source = CraftUiStateSource.production();
        UiSubscription subscription = source.subscribe(ignored -> {
            throw new IllegalStateException("render failed");
        });
        List<List<CraftRecipe>> following = new ArrayList<>();
        CraftStore.addRecipeListener(following::add);
        CraftRecipe recipe = CraftScreenViewModelTest.recipe("safe_write");

        assertDoesNotThrow(() -> CraftStore.replaceRecipes(List.of(recipe)),
            "UI listener 失败不能反向破坏 authoritative Store 写入");
        assertEquals(List.of(recipe), CraftStore.recipes());
        assertEquals(List.of(List.of(recipe)), following,
            "失败的 UI listener 之后，Store 的其他 listener 仍必须收到变化");
        subscription.close();
    }

    private static void resetStores() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
        InventoryStateStore.resetForTests();
        SkillSetStore.resetForTests();
    }
}
