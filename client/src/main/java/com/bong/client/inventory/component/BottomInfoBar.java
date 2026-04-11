package com.bong.client.inventory.component;

import com.bong.client.inventory.model.InventoryModel;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.Locale;

public class BottomInfoBar extends BaseComponent {
    private static final int BAR_WIDTH = 360;
    private static final int BAR_HEIGHT = 14;
    private static final int BG_COLOR = 0xFF151515;
    private static final int TEXT_COLOR = 0xFFCCCCCC;
    private static final int OVERWEIGHT_COLOR = 0xFFFF4444;

    private double currentWeight = 0;
    private double maxWeight = 50;
    private long spiritStones = 0;

    public BottomInfoBar() {
        this.sizing(Sizing.fixed(BAR_WIDTH), Sizing.fixed(BAR_HEIGHT));
    }

    public void updateFromModel(InventoryModel model) {
        this.currentWeight = model.currentWeight();
        this.maxWeight = model.maxWeight();
        this.spiritStones = model.spiritStones();
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        context.fill(x, y, x + width, y + BAR_HEIGHT, BG_COLOR);

        var textRenderer = MinecraftClient.getInstance().textRenderer;

        // Weight — left
        boolean overweight = currentWeight > maxWeight;
        String weightText = String.format(Locale.ROOT, "重量 %.1f/%.1f", currentWeight, maxWeight);
        int weightColor = overweight ? OVERWEIGHT_COLOR : TEXT_COLOR;
        context.drawTextWithShadow(textRenderer, Text.literal(weightText), x + 4, y + 3, weightColor);

        // Spirit stones — right
        String stonesText = "灵石: " + spiritStones;
        int stonesWidth = textRenderer.getWidth(stonesText);
        context.drawTextWithShadow(textRenderer, Text.literal(stonesText), x + width - stonesWidth - 4, y + 3, 0xFFFFD700);
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return BAR_WIDTH; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return BAR_HEIGHT; }
}
