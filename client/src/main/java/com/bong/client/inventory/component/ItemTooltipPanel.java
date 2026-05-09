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
    /**
     * 空面板/hint 默认高度，也是最小高度保证 icon 高度不被裁 + 常见 description 完整显示。
     * 统计当前所有 item description 最长 92 字符（≈ 46 汉字 ≈ 4 行全宽），top
     * 固定（padding + name + meta + optional status）最大 37 px，加 padding_bottom
     * 和 desc 行高估算约 81 px。112 给足余量；超出的长描述仍由动态 sizing 自动扩展。
     */
    private static final int DEFAULT_HEIGHT = 112;
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
        if (!formatStatusLine(item).isEmpty()) {
            needed += lineBlock;
        }
        if (item.forgeQuality() != null) {
            needed += lineBlock;
        }
        if (!item.forgeSideEffects().isEmpty()) {
            needed += lineBlock;
        }
        if (!item.alchemyLines().isEmpty()) {
            needed += lineBlock * item.alchemyLines().size();
        }

        // plan-armor-v1 §5：护甲矩阵（仅护甲类物品显示）。
        if (com.bong.client.combat.ArmorProfileStore.isArmor(item.itemId())) {
            needed += lineBlock * 2;
        }

        // top 部分至少保证 icon 高度（描述推到 icon 底部之下显示）。
        needed = Math.max(needed, ICON_MARGIN + ICON_SIZE);

        // description 用 TextRenderer.wrapLines 做真正的 word-wrap，按全宽计算。
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

        // 真元 / 耐久 —— 仅当 < 1.0 时显示，避免新玩家信息过载。
        String status = formatStatusLine(hoveredItem);
        if (!status.isEmpty()) {
            context.drawTextWithShadow(textRenderer, Text.literal(status), cx, cy, statusColor(hoveredItem));
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        if (hoveredItem.forgeQuality() != null) {
            StringBuilder forge = new StringBuilder(String.format(
                Locale.ROOT,
                "炼成 %.0f%%",
                hoveredItem.forgeQuality() * 100
            ));
            if (hoveredItem.forgeAchievedTier() != null) {
                forge.append(" · ").append(hoveredItem.forgeAchievedTier()).append("阶");
            }
            if (!hoveredItem.forgeColor().isEmpty()) {
                forge.append(" · ").append(forgeColorLabel(hoveredItem.forgeColor()));
            }
            context.drawTextWithShadow(textRenderer, Text.literal(forge.toString()), cx, cy, 0xFF88DDBB);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        if (!hoveredItem.forgeSideEffects().isEmpty()) {
            String sideEffects = "瑕疵 " + String.join("/", hoveredItem.forgeSideEffects());
            context.drawTextWithShadow(textRenderer, Text.literal(sideEffects), cx, cy, 0xFFDDAA66);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        for (String line : hoveredItem.alchemyLines()) {
            context.drawTextWithShadow(textRenderer, Text.literal(line), cx, cy, 0xFFE0B060);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        // plan-armor-v1 §5：护甲减免矩阵（WoundKind×系数）。
        com.bong.client.combat.ArmorProfileStore.ArmorMitigation mitigation =
            com.bong.client.combat.ArmorProfileStore.mitigationForItemId(hoveredItem.itemId());
        if (mitigation != null) {
            // Two compact rows: 斩/钝/刺 and 灼/震.
            String row1 = String.format(Locale.ROOT,
                "护甲 %s %s %s",
                renderMitigationCell("cut", mitigation.cut()),
                renderMitigationCell("blunt", mitigation.blunt()),
                renderMitigationCell("pierce", mitigation.pierce())
            );
            String row2 = String.format(Locale.ROOT,
                "     %s %s",
                renderMitigationCell("burn", mitigation.burn()),
                renderMitigationCell("concussion", mitigation.concussion())
            );

            context.drawTextWithShadow(textRenderer, Text.literal(row1), cx, cy, 0xFF88A0B0);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
            context.drawTextWithShadow(textRenderer, Text.literal(row2), cx, cy, 0xFF88A0B0);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        // Description —— 用 TextRenderer.wrapLines 做真正的 word-wrap（按字符宽度分行，不加 "…"）。
        // 为保证 wrap 宽度稳定，统一推到 icon 底部之下全宽显示，不再绕 icon 右侧。
        int iconBottom = y + ICON_MARGIN + ICON_SIZE;
        String desc = hoveredItem.description();
        if (!desc.isEmpty()) {
            cy = Math.max(cy, iconBottom);
            int maxWidth = PANEL_WIDTH - ICON_MARGIN * 2;
            for (var line : textRenderer.wrapLines(Text.literal(desc), maxWidth)) {
                if (cy > y + h - textRenderer.fontHeight - 2) break;
                context.drawTextWithShadow(textRenderer, line, descLeft, cy, 0xFFAAAAAA);
                cy += textRenderer.fontHeight + DESC_LINE_STEP;
            }
        }
    }

    private static String rarityLabel(String rarity) {
        return switch (rarity) {
            case "legendary" -> "传说";
            case "rare" -> "稀有";
            case "uncommon" -> "精良";
            default -> "普通";
        };
    }

    private static String forgeColorLabel(String color) {
        return switch (color) {
            case "Sharp" -> "锐";
            case "Heavy" -> "厚";
            case "Mellow" -> "醇";
            case "Solid" -> "实";
            case "Light" -> "轻";
            case "Intricate" -> "巧";
            case "Gentle" -> "柔";
            case "Insidious" -> "阴";
            case "Violent" -> "烈";
            case "Turbid" -> "浊";
            default -> color;
        };
    }

    public static String formatStatusLine(InventoryItem item) {
        if (item == null || item.isEmpty()) return "";

        StringBuilder status = new StringBuilder();
        if (item.spiritQuality() < 1.0) {
            String label = item.isBoneCoin() ? "封灵真元" : "纯度";
            status.append(String.format(Locale.ROOT, "%s %.0f%%", label, item.spiritQuality() * 100));
        }
        if (item.durability() < 1.0) {
            if (status.length() > 0) status.append("  ");
            status.append(String.format(Locale.ROOT, "耐久 %.0f%%", item.durability() * 100));
        }
        return status.toString();
    }

    private static int statusColor(InventoryItem item) {
        return (item.spiritQuality() < 0.3 || item.durability() < 0.3)
            ? 0xFFFF6666 : 0xFFAA8866;
    }

    private static String renderMitigationCell(String kind, float mitigation) {
        String label = com.bong.client.combat.ArmorProfileStore.kindLabel(kind);
        int pct = Math.round(mitigation * 100f);
        return label + "-" + pct + "%";
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return PANEL_WIDTH; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return currentHeight; }
}
