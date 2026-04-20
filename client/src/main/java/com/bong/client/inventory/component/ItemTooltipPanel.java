package com.bong.client.inventory.component;

import com.bong.client.inventory.model.InventoryItem;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.text.Text;

import java.util.Locale;

public class ItemTooltipPanel extends BaseComponent {
    private static final int PANEL_WIDTH = 196;
    /** 空面板/hint 默认高度，也是最小高度保证 icon 高度不被裁。 */
    private static final int DEFAULT_HEIGHT = 72;
    private static final int BG_COLOR = 0xCC181818;
    private static final int BORDER_COLOR = 0xFF3A3A3A;
    private static final int HINT_COLOR = 0x60AAAAAA;

    // Icon 占左上角一个正方形，文字从 icon 右边起。
    private static final int ICON_SIZE = 32;
    private static final int ICON_MARGIN = 4;
    private static final int TEXT_LEFT_OFFSET = ICON_MARGIN + ICON_SIZE + 4;
    private static final int PADDING_TOP = 4;
    private static final int PADDING_BOTTOM = 4;
    private static final int DESC_LINE_STEP = 1;
    private static final int BLOCK_LINE_STEP = 2;

    private InventoryItem hoveredItem;
    private int currentHeight = DEFAULT_HEIGHT;

    public ItemTooltipPanel() {
        this.sizing(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(DEFAULT_HEIGHT));
    }

    public void setHoveredItem(InventoryItem item) {
        this.hoveredItem = item;
        int required = computeRequiredHeight(item);
        if (required != currentHeight) {
            currentHeight = required;
            // owo-lib BaseComponent.sizing 是 Observable，改值会自动触发 notifyParentIfMounted，
            // parent FlowLayout 随之重新 inflate，新高度本轮或下一轮渲染即生效。
            this.sizing(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(currentHeight));
        }
    }

    private int computeRequiredHeight(InventoryItem item) {
        if (item == null || item.isEmpty()) return DEFAULT_HEIGHT;

        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;
        int lineBlock = textRenderer.fontHeight + BLOCK_LINE_STEP;

        // 顶部固定：padding + name + meta +（可选）status
        int needed = PADDING_TOP + lineBlock + lineBlock;
        if (item.spiritQuality() < 1.0 || item.durability() < 1.0) {
            needed += lineBlock;
        }

        // description 按全宽 word-wrap 估算（保守：忽略前几行可能挤 icon 右侧，
        // 实际绕 icon 时行数只会更少 → 估高一点没坏处）。
        if (!item.description().isEmpty()) {
            int maxWidth = PANEL_WIDTH - ICON_MARGIN * 2;
            int lines = textRenderer.wrapLines(Text.literal(item.description()), maxWidth).size();
            needed += lines * (textRenderer.fontHeight + DESC_LINE_STEP);
        }
        needed += PADDING_BOTTOM;

        return Math.max(DEFAULT_HEIGHT, needed);
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        int h = this.height;
        context.fill(x, y, x + PANEL_WIDTH, y + h, BG_COLOR);
        GridSlotComponent.drawSlotBorder(context, x, y, PANEL_WIDTH, h, BORDER_COLOR);

        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;

        if (hoveredItem == null || hoveredItem.isEmpty()) {
            String hint = "移动光标至物品查看详情";
            int hintX = x + (PANEL_WIDTH - textRenderer.getWidth(hint)) / 2;
            int hintY = y + (h - textRenderer.fontHeight) / 2;
            context.drawTextWithShadow(textRenderer, Text.literal(hint), hintX, hintY, HINT_COLOR);
            return;
        }

        // 左上角 icon —— 复用 GridSlotComponent.drawItemTexture（含内部 z=100 push + blend 设置）。
        GridSlotComponent.drawItemTexture(
            context, hoveredItem,
            x + ICON_MARGIN, y + ICON_MARGIN,
            ICON_SIZE, ICON_SIZE
        );

        int cy = y + PADDING_TOP;
        int cx = x + TEXT_LEFT_OFFSET;
        int descLeft = x + ICON_MARGIN;

        // Item name with rarity color
        context.drawTextWithShadow(textRenderer,
            Text.literal(hoveredItem.displayName()),
            cx, cy, hoveredItem.rarityColor());
        cy += textRenderer.fontHeight + BLOCK_LINE_STEP;

        // Rarity + size
        String meta = rarityLabel(hoveredItem.rarity())
            + " | " + hoveredItem.gridWidth() + "×" + hoveredItem.gridHeight()
            + " | " + String.format(Locale.ROOT, "%.1f", hoveredItem.weight()) + "kg";
        if (hoveredItem.stackCount() > 1) {
            meta += " | x" + hoveredItem.stackCount();
        }
        context.drawTextWithShadow(textRenderer, Text.literal(meta), cx, cy, 0xFF888888);
        cy += textRenderer.fontHeight + BLOCK_LINE_STEP;

        // 纯度 / 耐久 —— 仅当 < 1.0 时显示，避免新玩家信息过载。
        if (hoveredItem.spiritQuality() < 1.0 || hoveredItem.durability() < 1.0) {
            StringBuilder status = new StringBuilder();
            if (hoveredItem.spiritQuality() < 1.0) {
                status.append(String.format(Locale.ROOT, "纯度 %.0f%%", hoveredItem.spiritQuality() * 100));
            }
            if (hoveredItem.durability() < 1.0) {
                if (status.length() > 0) status.append("  ");
                status.append(String.format(Locale.ROOT, "耐久 %.0f%%", hoveredItem.durability() * 100));
            }
            int statusColor = (hoveredItem.spiritQuality() < 0.3 || hoveredItem.durability() < 0.3)
                ? 0xFFFF6666 : 0xFFAA8866;
            context.drawTextWithShadow(textRenderer, Text.literal(status.toString()), cx, cy, statusColor);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        // Description 过 icon 底部后改用全宽排版（左边距回到面板起点）。
        int iconBottom = y + ICON_MARGIN + ICON_SIZE;
        String desc = hoveredItem.description();
        if (!desc.isEmpty()) {
            while (!desc.isEmpty() && cy < y + h - textRenderer.fontHeight - 2) {
                boolean belowIcon = cy >= iconBottom;
                int lineLeft = belowIcon ? descLeft : cx;
                int lineMax = belowIcon ? (PANEL_WIDTH - ICON_MARGIN * 2) : (PANEL_WIDTH - TEXT_LEFT_OFFSET - ICON_MARGIN);
                String line = trimToWidth(textRenderer, desc, lineMax);
                context.drawTextWithShadow(textRenderer, Text.literal(line), lineLeft, cy, 0xFFAAAAAA);
                cy += textRenderer.fontHeight + DESC_LINE_STEP;
                desc = desc.substring(line.length()).trim();
            }
        }
    }

    private static String trimToWidth(TextRenderer renderer, String text, int maxWidth) {
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
    protected int determineVerticalContentSize(Sizing sizing) { return currentHeight; }
}
