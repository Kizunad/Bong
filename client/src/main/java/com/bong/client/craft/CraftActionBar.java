package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.skill.SkillSetSnapshot;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.text.Text;

import java.util.function.IntConsumer;

/** 紧凑操作条，数量可预设，材料由玩家逐项放入。 */
public final class CraftActionBar {
    private final FlowLayout root;
    private final ButtonComponent minusButton;
    private final ButtonComponent plusButton;
    private final ButtonComponent startButton;
    private final ButtonComponent returnButton;
    private final LabelComponent quantityLabel;
    private final Runnable onQuantityChanged;
    private int quantity = 1;

    public CraftActionBar(Runnable onReturn, IntConsumer onStart, Runnable onQuantityChanged) {
        this.onQuantityChanged = onQuantityChanged;
        root = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(22));
        root.gap(4);
        root.verticalAlignment(VerticalAlignment.CENTER);
        minusButton = button("-", "craft-minus", 20, () -> changeQuantity(-1));
        quantityLabel = Components.label(Text.literal("1"));
        quantityLabel.color(Color.ofArgb(0xFFE8DDC4));
        quantityLabel.horizontalSizing(Sizing.fixed(20));
        plusButton = button("+", "craft-plus", 20, () -> changeQuantity(1));
        returnButton = button("取回材料", "craft-return", 64, onReturn);
        startButton = button("制作", "craft-start", 76, () -> onStart.accept(quantity));
        root.child(minusButton).child(quantityLabel).child(plusButton).child(returnButton).child(startButton);
    }

    public FlowLayout root() { return root; }

    public int quantity() { return quantity; }

    public void availability(boolean available, boolean busy) {
        if (!available || busy) startButton.active(false);
        if (busy) {
            minusButton.active(false);
            plusButton.active(false);
            returnButton.active(false);
            startButton.setMessage(Text.literal("制作中"));
        } else if (!available) startButton.setMessage(Text.literal("工位不可用"));
    }

    public void refresh(CraftRecipe recipe, InventoryModel inventory, CraftSessionStateView session,
                        SkillSetSnapshot skills) {
        boolean active = session.active();
        int max = CraftInventoryCounter.maxCraftable(recipe, inventory);
        String status = recipe == null ? "未选配方" : !recipe.unlocked() ? "未解锁"
            : !recipe.skillSatisfied(skills) ? "技艺不足" : max < quantity ? "材料未齐" : "制作";
        minusButton.active(!active && quantity > 1);
        plusButton.active(!active && quantity < 64);
        returnButton.active(!active && !inventory.craftMaterials().isEmpty());
        startButton.active(!active && recipe != null && recipe.unlocked()
            && recipe.skillSatisfied(skills) && max >= quantity);
        startButton.setMessage(Text.literal(active ? "制作中" : status));
        startButton.tooltip(Text.literal(status));
    }

    private void changeQuantity(int delta) {
        quantity = Math.max(1, Math.min(64, quantity + delta));
        quantityLabel.text(Text.literal(Integer.toString(quantity)));
        onQuantityChanged.run();
    }

    private static ButtonComponent button(String text, String id, int width, Runnable action) {
        var button = Components.button(Text.literal(text), ignored -> action.run());
        button.id(id);
        button.sizing(Sizing.fixed(width), Sizing.fixed(20));
        button.textShadow(false);
        button.renderer(ButtonComponent.Renderer.flat(0xFF304347, 0xFF4C6464, 0xFF202D30));
        return button;
    }
}
