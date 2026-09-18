package com.bong.client.craft;

import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.CursorStyle;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.text.Text;

/** 产物说明折叠在材料之后，不占用制作操作的固定空间。 */
public final class CraftOutputPreview {
    private final FlowLayout root;
    private CraftRecipe recipe;
    private boolean expanded;
    private int width = CraftScreenLayout.RIGHT_W;
    private int flashTicks;

    public CraftOutputPreview() {
        root = Containers.verticalFlow(Sizing.fixed(width), Sizing.content());
        root.padding(Insets.of(2));
        root.gap(4);
    }

    public FlowLayout root() { return root; }

    public void layout(int width) {
        this.width = Math.max(1, width);
        root.horizontalSizing(Sizing.fixed(this.width));
        for (var child : root.children()) {
            if (child instanceof LabelComponent label) label.maxWidth(Math.max(1, this.width - 4));
        }
    }

    public void refresh(CraftRecipe recipe, int flashTicks) {
        this.recipe = recipe;
        this.flashTicks = flashTicks;
        rebuild();
    }

    private void rebuild() {
        root.clearChildren();
        if (recipe == null) return;
        var heading = label((expanded ? "收起产物 · " : "产物 · ") + CraftRecipeFilter.displayName(recipe),
            flashTicks > 0 ? 0xFFEACB91 : 0xFFABBDB7);
        heading.cursorStyle(CursorStyle.HAND);
        heading.mouseDown().subscribe((x, y, button) -> {
            if (button != 0) return false;
            expanded = !expanded;
            rebuild();
            return true;
        });
        root.child(heading);
        if (!expanded) return;
        var icon = new CraftMaterialSlotComponent();
        icon.setContent(recipe.unlocked() ? recipe.outputTemplate() : "unknown_recipe", recipe.outputCount(), true);
        root.child(icon);
        root.child(label(recipe.category().displayName() + " · " + String.format("%.1fs", recipe.timeTicks() / 20.0),
            0xFFA8B5AC));
        if (!recipe.unlocked()) root.child(label(CraftRecipeFilter.unlockHint(recipe), 0xFFBBAA94));
        else {
            for (String requirement : recipe.requirements().humanLines()) {
                root.child(label(requirement, 0xFFBBAA94));
            }
        }
    }

    private LabelComponent label(String text, int color) {
        var label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        label.maxWidth(Math.max(1, width - 4));
        return label;
    }
}
