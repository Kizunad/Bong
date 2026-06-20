package com.bong.client.craft;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-workbench-recipes-v1 P3.1 -- WorkbenchScreen 单元测试。
 *
 * <p>测试焦点：</p>
 * <ul>
 *   <li>station 过滤：workbenchRecipes() 只返回 station="workbench" 的配方</li>
 *   <li>CraftRecipe.station 字段正确传递</li>
 *   <li>CraftRecipe.isWorkbenchRecipe() 正确判断</li>
 *   <li>WorkbenchScreen 标题正确</li>
 *   <li>边界：空 recipe 列表、全手搓配方、全制作台配方</li>
 * </ul>
 */
class WorkbenchScreenTest {

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

    // ---- station 字段 ----

    @Test
    void stationFieldNullMeansHandcraft() {
        CraftRecipe handcraft = sampleRecipe("hand.pick", CraftCategory.TOOL, true, null);
        assertFalse(handcraft.isWorkbenchRecipe(),
            "station=null 应为手搓配方(isWorkbenchRecipe=false)，实际 station=" + handcraft.station());
        assertEquals(null, handcraft.station(),
            "station=null 表示手搓，实际 station=" + handcraft.station());
    }

    @Test
    void stationFieldWorkbenchMeansWorkbench() {
        CraftRecipe wb = sampleRecipe("workbench.pick", CraftCategory.TOOL, true, "workbench");
        assertTrue(wb.isWorkbenchRecipe(),
            "station=\"workbench\" 应为制作台配方(isWorkbenchRecipe=true)，实际 station=" + wb.station());
        assertEquals("workbench", wb.station(),
            "station 值应为 \"workbench\"，实际 station=" + wb.station());
    }

    @Test
    void stationFieldOtherMeansNotWorkbench() {
        CraftRecipe forge = sampleRecipe("forge.sword", CraftCategory.TOOL, true, "forge");
        assertFalse(forge.isWorkbenchRecipe(),
            "station=\"forge\" 应不是制作台配方(isWorkbenchRecipe=false)，实际 station=" + forge.station());
    }

    // ---- 手搓 vs 制作台 station 作用域互斥（防串台：手搓台只展示 isHandcraft，
    //      制作台屏只展示 isWorkbenchRecipe）----

    @Test
    void handcraftAndWorkbenchScopesAreMutuallyExclusive() {
        CraftRecipe handcraft = sampleRecipe("hand.wood_handle", CraftCategory.TOOL, true, null);
        CraftRecipe workbench = sampleRecipe("wb.stone_knife", CraftCategory.TOOL, true, "workbench");

        assertTrue(handcraft.isHandcraft(),
            "station=null 应为手搓配方(isHandcraft=true)，手搓台只展示它");
        assertFalse(handcraft.isWorkbenchRecipe(),
            "手搓配方不应被制作台屏展示(isWorkbenchRecipe=false)");

        assertTrue(workbench.isWorkbenchRecipe(),
            "station=\"workbench\" 应只在制作台屏展示");
        assertFalse(workbench.isHandcraft(),
            "制作台配方不应串进手搓台(isHandcraft=false)——这正是 stone_knife 误现于手搓台的根因");
    }

    @Test
    void recipeListWidgetRejectsNullStationScope() {
        // requireNonNull 在 owo 构件创建前，故 null 分支可脱离 MC 环境直接验证。
        assertThrows(NullPointerException.class,
            () -> new CraftRecipeListWidget(id -> {}, null),
            "stationScope=null 应在构造时即抛 NPE（fail-fast），而非延迟到 refresh() 的 filter 处");
    }

    @Test
    void forgeStationRecipeShowsInNeitherCraftScreen() {
        CraftRecipe forge = sampleRecipe("forge.sword", CraftCategory.TOOL, true, "forge");
        assertFalse(forge.isHandcraft(),
            "station=\"forge\" 非手搓，不应出现在手搓台，实际 station=" + forge.station());
        assertFalse(forge.isWorkbenchRecipe(),
            "station=\"forge\" 非制作台，不应出现在制作台屏，实际 station=" + forge.station());
    }

    @Test
    void withUnlockedPreservesStation() {
        CraftRecipe original = sampleRecipe("wb.tool", CraftCategory.TOOL, false, "workbench");
        CraftRecipe unlocked = original.withUnlocked(true);
        assertTrue(unlocked.unlocked(), "withUnlocked(true) 应解锁配方");
        assertEquals("workbench", unlocked.station(),
            "withUnlocked() 必须保留 station 字段，实际 station=" + unlocked.station());
        assertTrue(unlocked.isWorkbenchRecipe(),
            "withUnlocked() 后 isWorkbenchRecipe() 应保持 true");
    }

    @Test
    void withUnlockedNoOpReturnsSameInstanceWhenAlreadyUnlocked() {
        CraftRecipe original = sampleRecipe("wb.tool", CraftCategory.TOOL, true, "workbench");
        CraftRecipe same = original.withUnlocked(true);
        assertTrue(same == original,
            "withUnlocked(true) 对已解锁配方应返回同一实例(引用相同)以避免不必要的 Store 更新");
    }

    // ---- workbenchRecipes() 过滤 ----

    @Test
    void workbenchRecipesFiltersOnlyWorkbenchStation() {
        CraftStore.replaceRecipes(List.of(
            sampleRecipe("hand.knife", CraftCategory.TOOL, true, null),
            sampleRecipe("wb.splint", CraftCategory.MISC, true, "workbench"),
            sampleRecipe("wb.rope", CraftCategory.MISC, true, "workbench"),
            sampleRecipe("forge.blade", CraftCategory.TOOL, true, "forge")
        ));
        List<CraftRecipe> result = WorkbenchScreen.workbenchRecipes();
        assertEquals(2, result.size(),
            "workbenchRecipes() 应只返回 station=\"workbench\" 的配方(期望2)，实际 " + result.size());
        assertTrue(result.stream().allMatch(CraftRecipe::isWorkbenchRecipe),
            "所有返回配方的 isWorkbenchRecipe() 必须为 true");
        assertEquals("wb.splint", result.get(0).id());
        assertEquals("wb.rope", result.get(1).id());
    }

    @Test
    void workbenchRecipesReturnsEmptyWhenNoWorkbenchRecipes() {
        CraftStore.replaceRecipes(List.of(
            sampleRecipe("hand.pick", CraftCategory.TOOL, true, null),
            sampleRecipe("hand.axe", CraftCategory.TOOL, true, null)
        ));
        List<CraftRecipe> result = WorkbenchScreen.workbenchRecipes();
        assertTrue(result.isEmpty(),
            "无 station=\"workbench\" 配方时应返回空列表，实际 " + result.size());
    }

    @Test
    void workbenchRecipesReturnsEmptyWhenStoreEmpty() {
        List<CraftRecipe> result = WorkbenchScreen.workbenchRecipes();
        assertTrue(result.isEmpty(),
            "CraftStore 为空时 workbenchRecipes() 应返回空列表");
    }

    @Test
    void workbenchRecipesReturnsAllWhenAllAreWorkbench() {
        CraftStore.replaceRecipes(List.of(
            sampleRecipe("wb.a", CraftCategory.TOOL, true, "workbench"),
            sampleRecipe("wb.b", CraftCategory.MISC, false, "workbench"),
            sampleRecipe("wb.c", CraftCategory.CONTAINER, true, "workbench")
        ));
        List<CraftRecipe> result = WorkbenchScreen.workbenchRecipes();
        assertEquals(3, result.size(),
            "全部为制作台配方时应返回全部，实际 " + result.size());
    }

    @Test
    void workbenchRecipesIncludesLockedRecipes() {
        CraftStore.replaceRecipes(List.of(
            sampleRecipe("wb.locked", CraftCategory.MISC, false, "workbench"),
            sampleRecipe("wb.unlocked", CraftCategory.MISC, true, "workbench")
        ));
        List<CraftRecipe> result = WorkbenchScreen.workbenchRecipes();
        assertEquals(2, result.size(),
            "workbenchRecipes() 应包含已锁定和已解锁的制作台配方");
        assertFalse(result.get(0).unlocked(), "第一条应为已锁定配方");
        assertTrue(result.get(1).unlocked(), "第二条应为已解锁配方");
    }

    // ---- title ----

    @Test
    void workbenchScreenTitleIsCorrect() {
        assertNotNull(WorkbenchScreen.TITLE,
            "WorkbenchScreen.TITLE 不应为 null");
        assertEquals("末法制作台", WorkbenchScreen.TITLE.getString(),
            "标题应为「末法制作台」，实际 " + WorkbenchScreen.TITLE.getString());
    }

    // ---- CraftRecipe backward compatibility ----

    @Test
    void oldConstructorWithoutStationDefaultsToNull() {
        CraftRecipe recipe = new CraftRecipe(
            "old.recipe", CraftCategory.MISC, "旧配方",
            List.of(new CraftRecipe.MaterialEntry("herb", 1)),
            0.0, 60L, "output", 1,
            CraftRecipe.Requirements.NONE, true
        );
        assertEquals(null, recipe.station(),
            "旧构造器(无 station 参数)应默认 station=null，实际 " + recipe.station());
        assertFalse(recipe.isWorkbenchRecipe(),
            "旧构造器配方不应被判为制作台配方");
    }

    @Test
    void newConstructorWithStationPreservesValue() {
        CraftRecipe recipe = new CraftRecipe(
            "new.recipe", CraftCategory.MISC, "新配方",
            List.of(new CraftRecipe.MaterialEntry("herb", 1)),
            0.0, 60L, "output", 1,
            CraftRecipe.Requirements.NONE, true, "workbench"
        );
        assertEquals("workbench", recipe.station(),
            "新构造器应保留 station=\"workbench\"");
        assertTrue(recipe.isWorkbenchRecipe());
    }

    // ---- category isolation (workbench recipes span all categories) ----

    @Test
    void workbenchRecipesSpanMultipleCategories() {
        CraftStore.replaceRecipes(List.of(
            sampleRecipe("wb.tool", CraftCategory.TOOL, true, "workbench"),
            sampleRecipe("wb.container", CraftCategory.CONTAINER, true, "workbench"),
            sampleRecipe("wb.armor", CraftCategory.ARMOR_CRAFT, true, "workbench"),
            sampleRecipe("wb.poison", CraftCategory.POISON_POWDER, true, "workbench"),
            sampleRecipe("wb.anqi", CraftCategory.ANQI_CARRIER, true, "workbench"),
            sampleRecipe("hand.tool", CraftCategory.TOOL, true, null)
        ));
        List<CraftRecipe> result = WorkbenchScreen.workbenchRecipes();
        assertEquals(5, result.size(),
            "workbenchRecipes() 应返回 5 条跨多类别的制作台配方，实际 " + result.size());
    }

    // ---- helper ----

    private static CraftRecipe sampleRecipe(
        String id, CraftCategory category, boolean unlocked, String station
    ) {
        return new CraftRecipe(
            id, category, "示例 " + id,
            List.of(new CraftRecipe.MaterialEntry("herb", 2)),
            5.0, 120L, "output_" + id, 1,
            CraftRecipe.Requirements.NONE, unlocked, station
        );
    }
}
