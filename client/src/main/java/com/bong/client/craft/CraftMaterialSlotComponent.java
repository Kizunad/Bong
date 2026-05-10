package com.bong.client.craft;

import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.model.InventoryItem;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

/** 44px craft 材料/产物槽，复用现有物品图标注册表。 */
public final class CraftMaterialSlotComponent extends BaseComponent {
    private static final int BG = 0xFF1E1E1E;
    private static final int BORDER_OK = 0xFF3A6A3A;
    private static final int BORDER_MISSING = 0xFF6A3A3A;
    private static final int BORDER_EMPTY = 0xFF3A3A50;

    private String itemId = "";
    private int count = 0;
    private boolean sufficient = true;

    public CraftMaterialSlotComponent() {
        sizing(Sizing.fixed(CraftScreenLayout.MATERIAL_SLOT_SIZE), Sizing.fixed(CraftScreenLayout.MATERIAL_SLOT_SIZE));
    }

    public void setContent(String itemId, int count, boolean sufficient) {
        this.itemId = itemId == null ? "" : itemId;
        this.count = Math.max(0, count);
        this.sufficient = sufficient;
    }

    public void clearContent() {
        this.itemId = "";
        this.count = 0;
        this.sufficient = true;
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        int size = CraftScreenLayout.MATERIAL_SLOT_SIZE;
        context.fill(x, y, x + size, y + size, BG);
        int border = itemId.isEmpty() ? BORDER_EMPTY : (sufficient ? BORDER_OK : BORDER_MISSING);
        drawBorder(context, x, y, size, size, hovered ? 0xFFB0B0B0 : border);
        if (!itemId.isEmpty()) {
            InventoryItem item = InventoryItem.create(itemId, itemId, 1, 1, 1.0, "common", "");
            GridSlotComponent.drawItemTexture(context, item, x + 5, y + 4, size - 10, size - 12);
            String text = "x" + count;
            var renderer = MinecraftClient.getInstance().textRenderer;
            int tx = x + size - renderer.getWidth(text) - 3;
            int ty = y + size - renderer.fontHeight - 2;
            context.drawTextWithShadow(renderer, Text.literal(text), tx, ty, sufficient ? 0xFFE8FFE0 : 0xFFFF8080);
        }
    }

    private static void drawBorder(OwoUIDrawContext context, int x, int y, int w, int h, int color) {
        context.fill(x, y, x + w, y + 1, color);
        context.fill(x, y + h - 1, x + w, y + h, color);
        context.fill(x, y + 1, x + 1, y + h - 1, color);
        context.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) {
        return CraftScreenLayout.MATERIAL_SLOT_SIZE;
    }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) {
        return CraftScreenLayout.MATERIAL_SLOT_SIZE;
    }
}
