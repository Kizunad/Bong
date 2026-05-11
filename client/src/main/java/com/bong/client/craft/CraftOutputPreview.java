package com.bong.client.craft;

import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import net.minecraft.text.Text;

/** 右栏产物预览，只展示产物属性与说明。 */
public final class CraftOutputPreview {
    private final FlowLayout root;

    public CraftOutputPreview() {
        root = Containers.verticalFlow(Sizing.fixed(CraftScreenLayout.RIGHT_W), Sizing.fill(100));
        root.surface(Surface.flat(0xFF14141C).and(Surface.outline(0xFF3A3A50)));
        root.padding(Insets.of(5));
        root.gap(3);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        refresh(null, 0);
    }

    public FlowLayout root() {
        return root;
    }

    public void refresh(CraftRecipe recipe, int flashTicks) {
        root.clearChildren();
        if (recipe == null) {
            root.child(label("产物预览", 0xFFE8DDC4));
            root.child(label("选择配方后显示产物", 0xFF888888));
            return;
        }
        String displayName = CraftRecipeFilter.displayName(recipe);
        root.child(label("产物预览", 0xFFE8DDC4));
        CraftMaterialSlotComponent outputSlot = new CraftMaterialSlotComponent();
        outputSlot.setContent(recipe.unlocked() ? recipe.outputTemplate() : "unknown_recipe", recipe.outputCount(), true);
        outputSlot.tooltip(Text.literal(recipe.unlocked()
            ? recipe.outputTemplate() + " x" + recipe.outputCount()
            : CraftRecipeFilter.unlockHint(recipe)));
        root.child(outputSlot);
        root.child(label((flashTicks > 0 ? "§6✦ " : "") + displayName, recipe.unlocked() ? 0xFFFFFFFF : 0xFF777777));
        root.child(label(recipe.category().displayName(), 0xFFB8B8C8));
        root.child(label(String.format("制作 %.1fs", recipe.timeTicks() / 20.0), 0xFFA8A8B8));
        root.child(separator());
        for (String line : attributeLines(recipe)) {
            root.child(label(line, 0xFFE0E0EA));
        }
        root.child(separator());
        root.child(label(descriptionLine(recipe), 0xFFA8A8B8));
    }

    private static String[] attributeLines(CraftRecipe recipe) {
        return switch (recipe.category()) {
            case ANQI_CARRIER -> new String[] {"投掷载体  +" + recipe.outputCount(), "消耗后结算载体质量"};
            case DUGU_POTION -> new String[] {"药性  待鉴定", "可接毒功 / 丹药流程"};
            case TUIKE_SKIN -> new String[] {"伪装  可装备", "损耗随战斗结算"};
            case ZHENFA_TRAP -> new String[] {"阵件  可埋设", "触发后交由阵法系统"};
            case TOOL -> new String[] {"凡器  可使用", "耐久随采集/战斗损耗"};
            case ARMOR_CRAFT -> new String[] {"甲胄  可装备", "耐久随战斗结算"};
            case CONTAINER -> new String[] {"装具  扩展背包", "重量影响行动负担"};
            case POISON_POWDER -> new String[] {"毒粉  研磨产出", "供附毒 / 消化流程消耗"};
            case MISC -> new String[] {"凡物  通用材料", "以物易物基础流通物"};
        };
    }

    private static String descriptionLine(CraftRecipe recipe) {
        if (!recipe.unlocked()) {
            return "未知配方：" + CraftRecipeFilter.unlockHint(recipe);
        }
        return switch (recipe.category()) {
            case TOOL -> "散修常用器具，优先保证材料不被误耗。";
            case DUGU_POTION -> "药性需另行鉴别，手搓只负责凡物制备。";
            case CONTAINER -> "装具类产物会影响背包承重与格位。";
            case POISON_POWDER -> "由毒丹研磨成粉，供双层附毒流程消耗。";
            default -> "可从当前背包材料直接起手制作。";
        };
    }

    private static FlowLayout separator() {
        FlowLayout line = Containers.horizontalFlow(Sizing.fill(96), Sizing.fixed(1));
        line.surface(Surface.flat(0xFF3A3A50));
        return line;
    }

    private static LabelComponent label(String text, int color) {
        LabelComponent label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        label.maxWidth(CraftScreenLayout.RIGHT_W - 16);
        return label;
    }
}
