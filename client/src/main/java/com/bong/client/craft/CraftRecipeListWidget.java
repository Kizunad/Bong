package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.component.TextBoxComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.CursorStyle;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.text.Text;

import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Consumer;
import java.util.function.Predicate;
import java.util.stream.Collectors;

/** 左栏配方列表：搜索、分类 tab、收藏置顶、锁定配方提示。 */
public final class CraftRecipeListWidget {
    private static final Surface SELECTED_SURFACE = Surface.flat(0x403A6A3A);

    private final FlowLayout root;
    private final FlowLayout rows;
    private final TextBoxComponent searchBox;
    private final Consumer<String> onSelected;
    private final Predicate<CraftRecipe> stationScope;
    private final Set<String> favorites = new LinkedHashSet<>();
    // LinkedHashMap：保留插入顺序，供 refresh() 的 id 序列 diff 判定使用（HashMap 顺序不稳定会误判）。
    private final Map<String, FlowLayout> rowById = new LinkedHashMap<>();
    private final Map<String, LabelComponent> labelById = new LinkedHashMap<>();

    private CraftCategory category;
    private String selectedId;
    /** 配方列表滚动视口高度(px)；BODY_H 减标题/搜索/分类 tab/padding 后约余 208px,取 200 留余量,约 10 行。 */
    private static final int LIST_VIEWPORT_HEIGHT = 200;

    private String query = "";
    private InventoryModel lastInventory = InventoryModel.empty();
    /** 上一次 refresh() 渲染出的行 id 序列（含顺序），用于判定是否需要 clearChildren 重建。 */
    private List<String> renderedIds = List.of();
    /** 是否已经跑过至少一次完整重建（含"无匹配配方"占位）。首次 refresh() 必须走重建路径，
     * 否则初始 recipes 为空时 renderedIds(List.of()) == nextIds(List.of()) 会被误判"无需重建"，
     * 导致占位 label 都不渲染。 */
    private boolean rowsBuilt = false;

    /** 旧签名：不限 station（向后兼容测试），实际屏幕应传 stationScope。 */
    public CraftRecipeListWidget(Consumer<String> onSelected) {
        this(onSelected, r -> true);
    }

    /**
     * @param stationScope 站台作用域过滤：手搓台传 {@code CraftRecipe::isHandcraft}，
     *     制作台屏传 {@code CraftRecipe::isWorkbenchRecipe}。避免两类配方互相串台。
     */
    public CraftRecipeListWidget(Consumer<String> onSelected, Predicate<CraftRecipe> stationScope) {
        this.onSelected = onSelected;
        // fail-fast：null 会在 refresh() 的 filter(stationScope) 处晚到地 NPE，构造时即拒。
        this.stationScope = java.util.Objects.requireNonNull(stationScope, "stationScope must not be null");
        root = Containers.verticalFlow(Sizing.fixed(CraftScreenLayout.LEFT_W), Sizing.fill(100));
        root.surface(Surface.flat(0xFF1A1814).and(Surface.outline(0xFF4A4030)));
        root.padding(Insets.of(4));
        root.gap(3);
        root.child(label("配方", 0xFFE8DDC4));

        searchBox = Components.textBox(Sizing.fixed(CraftScreenLayout.LEFT_W - 10));
        searchBox.text("");
        searchBox.onChanged().subscribe(value -> {
            query = value == null ? "" : value;
            refresh(lastInventory);
        });
        root.child(searchBox);
        root.child(buildCategoryTabs());

        FlowLayout rowContent = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        rowContent.gap(1);
        rows = rowContent;
        // viewport 高度必须 Sizing.fixed —— owo 里 Sizing.fill(100) 是"撑满父容器整高"(非剩余
        // 空间),viewport 会被解算到 ≥ 内容高度,scroll 判定无需滚动 → 滚动条钉死拖不动(配方
        // 22 条时尤其明显)。仿能滚的先例 TechniquesTabPanel(固定 viewport)。
        var scroll = Containers.verticalScroll(Sizing.fill(100), Sizing.fixed(LIST_VIEWPORT_HEIGHT), rowContent);
        scroll.scrollbarThiccness(3);
        root.child(scroll);
    }

    public FlowLayout root() {
        return root;
    }

    public void setSelectedId(String selectedId) {
        this.selectedId = selectedId;
    }

    public void refresh(InventoryModel inventory) {
        lastInventory = inventory == null ? InventoryModel.empty() : inventory;
        List<CraftRecipe> scoped = CraftStore.recipes().stream()
            .filter(stationScope)
            .collect(Collectors.toList());
        List<CraftRecipe> recipes = CraftRecipeFilter.filter(scoped, category, query, favorites);
        List<String> nextIds = recipes.stream().map(CraftRecipe::id).toList();

        if (rowsBuilt && !needsRebuild(renderedIds, nextIds)) {
            // id 序列（含顺序）完全一致 → 原地更新每行文案/颜色/tooltip/选中态，不 clearChildren。
            // owo ScrollContainer.layout() 会在内容清空瞬间把 maxScroll 算成 0 并 clamp scrollOffset
            // 回 0，随后行加回来 offset 也回不来 —— 这是"滚动条自动回弹到顶部"的根因，见 PR 描述。
            for (CraftRecipe recipe : recipes) {
                FlowLayout row = rowById.get(recipe.id());
                LabelComponent text = labelById.get(recipe.id());
                if (row == null || text == null) {
                    continue; // 理论不可达：needsRebuild=false 已保证 id 集合与 rowById 一致
                }
                applyRowContent(row, text, recipe, lastInventory);
            }
            return;
        }

        rows.clearChildren();
        rowById.clear();
        labelById.clear();
        rowsBuilt = true;
        if (recipes.isEmpty()) {
            rows.child(label("无匹配配方", 0xFF888888));
            renderedIds = List.of();
            return;
        }
        for (CraftRecipe recipe : recipes) {
            FlowLayout r = row(recipe, lastInventory);
            rowById.put(recipe.id(), r);
            rows.child(r);
        }
        renderedIds = nextIds;
    }

    /**
     * 判定配方 id 序列是否发生结构性变化（新增/删除/重排/空 ↔ 非空）。
     * 序列完全一致（含顺序）时无需重建行，可走原地更新路径以保留滚动位置。
     */
    static boolean needsRebuild(List<String> currentIds, List<String> nextIds) {
        return !currentIds.equals(nextIds);
    }

    /** 把配方最新状态（收藏/锁定/可做数量/选中态）写入已存在的行，不新建/不重排组件树。 */
    private void applyRowContent(FlowLayout row, LabelComponent text, CraftRecipe recipe, InventoryModel inventory) {
        String fav = favorites.contains(recipe.id()) ? "★" : " ";
        String lock = recipe.unlocked() ? " " : "🔒";
        int max = CraftInventoryCounter.maxCraftable(recipe, inventory);
        String count = recipe.unlocked() ? (max > 0 ? " §a" + max : " §8-") : " §8?";
        text.text(Text.literal(fav + lock + CraftRecipeFilter.displayName(recipe) + count));
        text.color(Color.ofArgb(recipe.unlocked() ? 0xFFE8DDC4 : 0xFF777777));
        row.tooltip(Text.literal(recipe.unlocked()
            ? recipe.displayName() + " · 右键收藏"
            : "??? · " + CraftRecipeFilter.unlockHint(recipe)));
        row.surface(recipe.id().equals(selectedId) ? SELECTED_SURFACE : Surface.BLANK);
    }

    private void selectRow(String recipeId, InventoryModel inventory) {
        FlowLayout prev = selectedId != null ? rowById.get(selectedId) : null;
        if (prev != null) {
            prev.surface(Surface.BLANK);
        }
        selectedId = recipeId;
        FlowLayout next = rowById.get(recipeId);
        if (next != null) {
            next.surface(SELECTED_SURFACE);
        }
        if (onSelected != null) {
            onSelected.accept(recipeId);
        }
    }

    private FlowLayout buildCategoryTabs() {
        FlowLayout wrap = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        wrap.gap(2);
        FlowLayout first = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        first.gap(2);
        first.child(tab("全", null));
        first.child(tab("暗", CraftCategory.ANQI_CARRIER));
        first.child(tab("汤", CraftCategory.DUGU_POTION));
        first.child(tab("皮", CraftCategory.TUIKE_SKIN));
        FlowLayout second = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        second.gap(2);
        second.child(tab("阵", CraftCategory.ZHENFA_TRAP));
        second.child(tab("器", CraftCategory.TOOL));
        second.child(tab("容", CraftCategory.CONTAINER));
        second.child(tab("杂", CraftCategory.MISC));
        wrap.child(first);
        wrap.child(second);
        return wrap;
    }

    private LabelComponent tab(String text, CraftCategory next) {
        LabelComponent label = label(text, 0xFFE8DDC4);
        label.cursorStyle(CursorStyle.HAND);
        label.tooltip(Text.literal(next == null ? "全部配方" : next.displayName()));
        label.mouseDown().subscribe((x, y, button) -> {
            if (button == 0) {
                category = next;
                refresh(lastInventory);
                return true;
            }
            return false;
        });
        return label;
    }

    private FlowLayout row(CraftRecipe recipe, InventoryModel inventory) {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(18));
        row.verticalAlignment(VerticalAlignment.CENTER);
        row.padding(Insets.of(1, 2, 2, 2));
        LabelComponent text = label("", 0xFFE8DDC4);
        text.maxWidth(CraftScreenLayout.LEFT_W - 12);
        row.child(text);
        row.cursorStyle(CursorStyle.HAND);
        applyRowContent(row, text, recipe, inventory);
        labelById.put(recipe.id(), text);
        row.mouseDown().subscribe((x, y, button) -> {
            if (button == 0) {
                selectRow(recipe.id(), lastInventory);
                return true;
            }
            if (button == 1) {
                if (!favorites.remove(recipe.id())) {
                    favorites.add(recipe.id());
                }
                // 用 lastInventory 而非闭包捕获的 inventory：后者是本行构建瞬间的旧快照，
                // 若在此之前已发生过原地更新（diff 路径不重建行/不重订阅），捕获值会 stale。
                refresh(lastInventory);
                return true;
            }
            return false;
        });
        return row;
    }

    private static LabelComponent label(String text, int color) {
        LabelComponent label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        return label;
    }
}
