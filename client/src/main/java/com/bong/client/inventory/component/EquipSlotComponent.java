package com.bong.client.inventory.component;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

public class EquipSlotComponent extends BaseComponent {
    public static final int SLOT_SIZE = 28;
    private static final int BG_COLOR = 0xFF1E1E1E;
    private static final int BORDER_COLOR = 0xFF4A4A4A;
    private static final int EMPTY_LABEL_COLOR = 0x60999999;
    private static final int HOVER_BORDER_COLOR = 0xFF999999;

    private final EquipSlotType slotType;
    private InventoryItem item;
    private GridSlotComponent.HighlightState highlightState = GridSlotComponent.HighlightState.NONE;

    public EquipSlotComponent(EquipSlotType slotType) {
        this.slotType = slotType;
        this.sizing(Sizing.fixed(SLOT_SIZE), Sizing.fixed(SLOT_SIZE));
    }

    public EquipSlotType slotType() { return slotType; }
    public InventoryItem item() { return item; }
    public void setItem(InventoryItem item) { this.item = item; }
    public void clearItem() { this.item = null; }

    public void setHighlightState(GridSlotComponent.HighlightState state) {
        this.highlightState = state;
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        context.fill(x, y, x + SLOT_SIZE, y + SLOT_SIZE, BG_COLOR);

        int borderColor = hovered ? HOVER_BORDER_COLOR : BORDER_COLOR;
        GridSlotComponent.drawSlotBorder(context, x, y, SLOT_SIZE, SLOT_SIZE, borderColor);

        switch (highlightState) {
            case VALID -> context.fill(x + 1, y + 1, x + SLOT_SIZE - 1, y + SLOT_SIZE - 1, 0x3300CC44);
            case INVALID -> context.fill(x + 1, y + 1, x + SLOT_SIZE - 1, y + SLOT_SIZE - 1, 0x33CC2222);
            case DIMMED -> context.fill(x + 1, y + 1, x + SLOT_SIZE - 1, y + SLOT_SIZE - 1, 0x66000000);
            default -> {}
        }

        if (item != null) {
            GridSlotComponent.drawItemTexture(context, item, x + 2, y + 2, SLOT_SIZE - 4, SLOT_SIZE - 4);
        } else {
            var textRenderer = MinecraftClient.getInstance().textRenderer;
            String label = slotType.displayName().substring(0, 1); // Single char: 头/甲/腿/鞋/右/左/双
            int textWidth = textRenderer.getWidth(label);
            int tx = x + (SLOT_SIZE - textWidth) / 2;
            int ty = y + (SLOT_SIZE - textRenderer.fontHeight) / 2;
            context.drawTextWithShadow(textRenderer, Text.literal(label), tx, ty, EMPTY_LABEL_COLOR);
        }
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return SLOT_SIZE; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return SLOT_SIZE; }
}
