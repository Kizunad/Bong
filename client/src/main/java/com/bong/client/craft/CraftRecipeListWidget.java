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

import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.function.Consumer;

/** 左栏配方列表：搜索、分类 tab、收藏置顶、锁定配方提示。 */
public final class CraftRecipeListWidget {
    private final FlowLayout root;
    private final FlowLayout rows;
    private final TextBoxComponent searchBox;
    private final Consumer<String> onSelected;
    private final Set<String> favorites = new LinkedHashSet<>();

    private CraftCategory category;
    private String selectedId;
    private String query = "";
    private InventoryModel lastInventory = InventoryModel.empty();

    public CraftRecipeListWidget(Consumer<String> onSelected) {
        this.onSelected = onSelected;
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
        var scroll = Containers.verticalScroll(Sizing.fill(100), Sizing.fill(100), rowContent);
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
        rows.clearChildren();
        List<CraftRecipe> recipes = CraftRecipeFilter.filter(CraftStore.recipes(), category, query, favorites);
        if (recipes.isEmpty()) {
            rows.child(label("无匹配配方", 0xFF888888));
            return;
        }
        for (CraftRecipe recipe : recipes) {
            rows.child(row(recipe, inventory));
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
        if (recipe.id().equals(selectedId)) {
            row.surface(Surface.flat(0x403A6A3A));
        }
        String fav = favorites.contains(recipe.id()) ? "★" : " ";
        String lock = recipe.unlocked() ? " " : "🔒";
        int max = CraftInventoryCounter.maxCraftable(recipe, inventory);
        String count = recipe.unlocked() ? (max > 0 ? " §a" + max : " §8-") : " §8?";
        LabelComponent text = label(fav + lock + CraftRecipeFilter.displayName(recipe) + count,
            recipe.unlocked() ? 0xFFE8DDC4 : 0xFF777777);
        text.maxWidth(CraftScreenLayout.LEFT_W - 12);
        row.child(text);
        row.cursorStyle(CursorStyle.HAND);
        row.tooltip(Text.literal(recipe.unlocked()
            ? recipe.displayName() + " · 右键收藏"
            : "??? · " + CraftRecipeFilter.unlockHint(recipe)));
        row.mouseDown().subscribe((x, y, button) -> {
            if (button == 0) {
                selectedId = recipe.id();
                if (onSelected != null) {
                    onSelected.accept(recipe.id());
                }
                refresh(inventory);
                return true;
            }
            if (button == 1) {
                if (!favorites.remove(recipe.id())) {
                    favorites.add(recipe.id());
                }
                refresh(inventory);
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
