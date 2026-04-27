package com.bong.client.inventory.component;

import com.bong.client.inventory.model.InventoryItem;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public class GridSlotComponent extends BaseComponent {
    public static final int CELL_SIZE = 28;
    private static final int ICON_SIZE = 128;
    private static final Identifier FALLBACK_ITEM_TEXTURE = new Identifier(
        "bong-client", "textures/gui/items/broken_artifact.png"
    );
    private static final Identifier FALLBACK_SCROLL_TEXTURE = new Identifier(
        "bong-client", "textures/gui/items/broken_artifact_scroll.png"
    );
    private static final Map<String, Identifier> TEXTURE_CACHE = new ConcurrentHashMap<>();

    // Gray palette
    private static final int BG_COLOR = 0xFF1E1E1E;
    private static final int BG_COLOR_ALT = 0xFF232323;
    private static final int BORDER_COLOR = 0xFF3A3A3A;
    private static final int HOVER_BORDER_COLOR = 0xFF888888;

    private final int row;
    private final int col;
    private InventoryItem item;
    private boolean isAnchor;
    private HighlightState highlightState = HighlightState.NONE;

    public enum HighlightState {
        NONE, VALID, INVALID, DIMMED
    }

    public GridSlotComponent(int row, int col) {
        this.row = row;
        this.col = col;
        this.sizing(Sizing.fixed(CELL_SIZE), Sizing.fixed(CELL_SIZE));
    }

    public int row() { return row; }
    public int col() { return col; }

    public void setItem(InventoryItem item, boolean isAnchor) {
        this.item = item;
        this.isAnchor = isAnchor;
    }

    public InventoryItem item() { return item; }
    public boolean isAnchor() { return isAnchor; }

    public void clearItem() {
        this.item = null;
        this.isAnchor = false;
    }

    public void setHighlightState(HighlightState state) {
        this.highlightState = state;
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        int bg = ((row + col) % 2 == 0) ? BG_COLOR : BG_COLOR_ALT;
        context.fill(x, y, x + CELL_SIZE, y + CELL_SIZE, bg);

        int borderColor = hovered ? HOVER_BORDER_COLOR : BORDER_COLOR;
        drawSlotBorder(context, x, y, CELL_SIZE, CELL_SIZE, borderColor);

        // Highlight overlay
        switch (highlightState) {
            case VALID -> context.fill(x + 1, y + 1, x + CELL_SIZE - 1, y + CELL_SIZE - 1, 0x3300CC44);
            case INVALID -> context.fill(x + 1, y + 1, x + CELL_SIZE - 1, y + CELL_SIZE - 1, 0x33CC2222);
            case DIMMED -> context.fill(x + 1, y + 1, x + CELL_SIZE - 1, y + CELL_SIZE - 1, 0x66000000);
            default -> {}
        }

        // Only draw 1×1 items here; multi-cell items are drawn by InspectScreen overlay
        if (item != null && isAnchor && item.gridWidth() == 1 && item.gridHeight() == 1) {
            drawItemTexture(context, item, x + 2, y + 2, CELL_SIZE - 4, CELL_SIZE - 4);
            drawItemOverlays(context, item, x, y, CELL_SIZE, CELL_SIZE);
        }
    }

    /** 在任意槽（含多格 overlay）上方绘制堆叠数与 quality/durability 色调。 */
    public static void drawItemOverlays(net.minecraft.client.gui.DrawContext context, InventoryItem item, int dx, int dy, int dw, int dh) {
        if (item == null || item.isEmpty()) return;

        // drawItemTexture 将贴图推到 z=100 并开启 depth test，overlay 必须抬到更高 z 才不会被遮挡。
        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(0, 0, 200);

        // 品质低于阈值 → 半透明灰叠层（越低越暗），暗示灵气流失或损耗。
        double quality = Math.min(item.spiritQuality(), item.durability());
        if (quality < 0.5) {
            // 0.5 → alpha 0x00, 0.0 → alpha 0x66
            int alpha = (int) ((0.5 - quality) * 2.0 * 0x66) & 0xFF;
            int overlay = (alpha << 24) | 0x222222;
            context.fill(dx + 1, dy + 1, dx + dw - 1, dy + dh - 1, overlay);
        }

        // 堆叠数字 —— 右下角（与原版 MC 物品栏一致，保证在贴图上层可见）。
        if (item.stackCount() > 1) {
            var tr = MinecraftClient.getInstance().textRenderer;
            String txt = String.valueOf(item.stackCount());
            int tx = dx + dw - tr.getWidth(txt) - 2;
            int ty = dy + dh - tr.fontHeight - 1;
            context.drawTextWithShadow(tr, Text.literal(txt), tx, ty, 0xFFFFFFFF);
        }

        matrices.pop();
    }

    public static void drawItemTexture(OwoUIDrawContext context, InventoryItem item, int dx, int dy, int dw, int dh) {
        if (item == null || item.isEmpty()) return;

        Identifier textureId = textureIdForItem(item);

        int fitSize = Math.min(dw, dh);
        int offsetX = (dw - fitSize) / 2;
        int offsetY = (dh - fitSize) / 2;

        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.enableDepthTest();

        var matrices = context.getMatrices();
        matrices.push();
        matrices.translate(dx + offsetX, dy + offsetY, 100);
        matrices.scale((float) fitSize / ICON_SIZE, (float) fitSize / ICON_SIZE, 1.0f);

        context.drawTexture(textureId, 0, 0, ICON_SIZE, ICON_SIZE, 0, 0, ICON_SIZE, ICON_SIZE, ICON_SIZE, ICON_SIZE);

        matrices.pop();
        RenderSystem.disableBlend();
    }

    public static Identifier textureIdForItem(InventoryItem item) {
        return textureIdForItemId(item == null ? "" : item.itemId());
    }

    public static Identifier textureIdForItemId(String itemId) {
        String normalized = itemId == null ? "" : itemId.trim();
        return TEXTURE_CACHE.computeIfAbsent(normalized, GridSlotComponent::resolveTextureIdForItemId);
    }

    private static Identifier resolveTextureIdForItemId(String itemId) {
        if (itemId.isEmpty()) {
            return FALLBACK_ITEM_TEXTURE;
        }

        Identifier candidate = new Identifier("bong-client", "textures/gui/items/" + itemId + ".png");
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.getResourceManager().getResource(candidate).isPresent()) {
            return candidate;
        }
        return fallbackTextureIdForItemId(itemId);
    }

    static Identifier fallbackTextureIdForItemId(String itemId) {
        return isScrollTextureCandidate(itemId) ? FALLBACK_SCROLL_TEXTURE : FALLBACK_ITEM_TEXTURE;
    }

    static boolean isScrollTextureCandidate(String itemId) {
        if (itemId == null || itemId.isBlank()) {
            return false;
        }
        return itemId.startsWith("skill_scroll_")
            || itemId.startsWith("recipe_scroll_")
            || itemId.startsWith("blueprint_scroll_")
            || itemId.startsWith("inscription_scroll_")
            || itemId.endsWith("_scroll");
    }

    static void drawSlotBorder(OwoUIDrawContext context, int x, int y, int w, int h, int color) {
        context.fill(x, y, x + w, y + 1, color);
        context.fill(x, y + h - 1, x + w, y + h, color);
        context.fill(x, y + 1, x + 1, y + h - 1, color);
        context.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return CELL_SIZE; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return CELL_SIZE; }
}
