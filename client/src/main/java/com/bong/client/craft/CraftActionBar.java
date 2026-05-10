package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.text.Text;

import java.util.function.IntConsumer;

/** 底部 32px 交互杠：一键填充、数量选择、开始制作。 */
public final class CraftActionBar {
    private final FlowLayout root;
    private final ButtonComponent fillButton;
    private final ButtonComponent minusButton;
    private final ButtonComponent plusButton;
    private final ButtonComponent startButton;
    private final LabelComponent quantityLabel;
    private final LabelComponent statusLabel;
    private final IntConsumer onStart;

    private CraftRecipe recipe;
    private int quantity = 1;
    private int maxQuantity = 0;

    public CraftActionBar(Runnable onFill, IntConsumer onStart) {
        this.onStart = onStart;
        root = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(CraftScreenLayout.ACTION_BAR_H));
        root.surface(Surface.flat(0xFF12121C).and(Surface.outline(0xFF3A3040)));
        root.padding(Insets.of(4));
        root.gap(5);
        root.verticalAlignment(VerticalAlignment.CENTER);

        fillButton = Components.button(Text.literal("一键填充"), b -> {
            if (onFill != null) {
                onFill.run();
            }
        });
        fillButton.sizing(Sizing.fixed(70), Sizing.fixed(20));
        root.child(fillButton);

        minusButton = Components.button(Text.literal("-"), b -> setQuantity(quantity - 1));
        minusButton.sizing(Sizing.fixed(20), Sizing.fixed(20));
        root.child(minusButton);
        quantityLabel = label("1", 0xFFE8DDC4);
        quantityLabel.horizontalSizing(Sizing.fixed(28));
        root.child(quantityLabel);
        plusButton = Components.button(Text.literal("+"), b -> setQuantity(quantity + 1));
        plusButton.sizing(Sizing.fixed(20), Sizing.fixed(20));
        root.child(plusButton);

        statusLabel = label("", 0xFFA8A8B8);
        statusLabel.horizontalSizing(Sizing.fill(100));
        root.child(statusLabel);

        startButton = Components.button(Text.literal("开始制作"), b -> {
            if (recipe != null && onStart != null) {
                onStart.accept(quantity);
            }
        });
        startButton.sizing(Sizing.fixed(86), Sizing.fixed(20));
        root.child(startButton);
    }

    public FlowLayout root() {
        return root;
    }

    public void refresh(CraftRecipe recipe, InventoryModel inventory, CraftSessionStateView session) {
        this.recipe = recipe;
        maxQuantity = CraftInventoryCounter.maxCraftable(recipe, inventory);
        if (quantity < 1) {
            quantity = 1;
        }
        if (maxQuantity > 0 && quantity > maxQuantity) {
            quantity = maxQuantity;
        }
        quantityLabel.text(Text.literal(String.valueOf(quantity)));

        boolean hasRecipe = recipe != null;
        boolean activeSession = session != null && session.active();
        boolean canStart = hasRecipe && recipe.unlocked() && maxQuantity > 0 && !activeSession;
        fillButton.active(hasRecipe && recipe.unlocked() && !activeSession);
        minusButton.active(maxQuantity > 0 && quantity > 1 && !activeSession);
        plusButton.active(maxQuantity > 0 && quantity < maxQuantity && !activeSession);
        startButton.active(canStart);
        statusLabel.text(Text.literal(statusText(recipe, maxQuantity, activeSession)));
        startButton.tooltip(Text.literal(canStart ? "起手搓 " + recipe.displayName() + " x" + quantity : statusText(recipe, maxQuantity, activeSession)));
    }

    public void setQuantityToMax() {
        if (maxQuantity > 0) {
            setQuantity(maxQuantity);
        }
    }

    public int quantity() {
        return quantity;
    }

    private void setQuantity(int next) {
        int upper = maxQuantity <= 0 ? 1 : maxQuantity;
        quantity = Math.max(1, Math.min(upper, next));
        quantityLabel.text(Text.literal(String.valueOf(quantity)));
    }

    private static String statusText(CraftRecipe recipe, int maxQuantity, boolean activeSession) {
        if (activeSession) {
            return "制作进行中";
        }
        if (recipe == null) {
            return "未选择配方";
        }
        if (!recipe.unlocked()) {
            return "配方未解锁";
        }
        if (maxQuantity <= 0) {
            return "材料不足";
        }
        return "可制作 x" + maxQuantity;
    }

    private static LabelComponent label(String text, int color) {
        LabelComponent label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        return label;
    }
}
