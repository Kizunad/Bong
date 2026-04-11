package com.bong.client.inventory.component;

import com.bong.client.inventory.model.InventoryItem;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.Locale;

public class ItemTooltipPanel extends BaseComponent {
    private static final int PANEL_WIDTH = 196;
    private static final int PANEL_HEIGHT = 58;
    private static final int BG_COLOR = 0xCC181818;
    private static final int BORDER_COLOR = 0xFF3A3A3A;
    private static final int HINT_COLOR = 0x60AAAAAA;

    private InventoryItem hoveredItem;

    public ItemTooltipPanel() {
        this.sizing(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(PANEL_HEIGHT));
    }

    public void setHoveredItem(InventoryItem item) {
        this.hoveredItem = item;
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        context.fill(x, y, x + PANEL_WIDTH, y + PANEL_HEIGHT, BG_COLOR);
        GridSlotComponent.drawSlotBorder(context, x, y, PANEL_WIDTH, PANEL_HEIGHT, BORDER_COLOR);

        var textRenderer = MinecraftClient.getInstance().textRenderer;

        if (hoveredItem == null || hoveredItem.isEmpty()) {
            String hint = "移动光标至物品查看详情";
            int hintX = x + (PANEL_WIDTH - textRenderer.getWidth(hint)) / 2;
            int hintY = y + (PANEL_HEIGHT - textRenderer.fontHeight) / 2;
            context.drawTextWithShadow(textRenderer, Text.literal(hint), hintX, hintY, HINT_COLOR);
            return;
        }

        int cy = y + 4;
        int cx = x + 4;

        // Item name with rarity color
        context.drawTextWithShadow(textRenderer,
            Text.literal(hoveredItem.displayName()),
            cx, cy, hoveredItem.rarityColor());
        cy += textRenderer.fontHeight + 2;

        // Rarity + size
        String meta = rarityLabel(hoveredItem.rarity())
            + " | " + hoveredItem.gridWidth() + "×" + hoveredItem.gridHeight()
            + " | " + String.format(Locale.ROOT, "%.1f", hoveredItem.weight()) + "kg";
        context.drawTextWithShadow(textRenderer, Text.literal(meta), cx, cy, 0xFF888888);
        cy += textRenderer.fontHeight + 2;

        // Description (truncate if needed)
        String desc = hoveredItem.description();
        if (!desc.isEmpty()) {
            // Simple word wrap at panel width
            int maxWidth = PANEL_WIDTH - 8;
            while (!desc.isEmpty() && cy < y + PANEL_HEIGHT - textRenderer.fontHeight - 2) {
                String line = trimToWidth(textRenderer, desc, maxWidth);
                context.drawTextWithShadow(textRenderer, Text.literal(line), cx, cy, 0xFFAAAAAA);
                cy += textRenderer.fontHeight + 1;
                desc = desc.substring(line.length()).trim();
            }
        }
    }

    private static String trimToWidth(net.minecraft.client.font.TextRenderer renderer, String text, int maxWidth) {
        if (renderer.getWidth(text) <= maxWidth) return text;
        for (int i = text.length() - 1; i > 0; i--) {
            String sub = text.substring(0, i) + "…";
            if (renderer.getWidth(sub) <= maxWidth) return sub;
        }
        return text.substring(0, 1);
    }

    private static String rarityLabel(String rarity) {
        return switch (rarity) {
            case "legendary" -> "传说";
            case "rare" -> "稀有";
            case "uncommon" -> "精良";
            default -> "普通";
        };
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return PANEL_WIDTH; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return PANEL_HEIGHT; }
}
