package com.bong.client.inspect;

import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.model.InventoryItem;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Objects;

public final class ItemInspectScreen extends Screen {
    private static final int PANEL_WIDTH = 260;
    private static final int ICON_SIZE = 80;
    private static final int TEXT_COLOR = 0xFFE8E0D0;
    private static final int MUTED_COLOR = 0xFFB0A898;
    private static final int BG = 0xE00D0F14;

    private final InventoryItem item;

    public ItemInspectScreen(InventoryItem item) {
        super(Text.literal(item == null ? "物品检视" : item.displayName()));
        this.item = Objects.requireNonNull(item, "item");
    }

    public InventoryItem item() {
        return item;
    }

    public static List<String> detailLines(InventoryItem item) {
        if (item == null || item.isEmpty()) {
            return List.of();
        }
        List<String> lines = new ArrayList<>();
        lines.add("名称: " + item.displayName());
        lines.add("稀有度: " + item.rarity());
        lines.add("品质: " + percent(item.spiritQuality()));
        lines.add("重量: " + String.format(Locale.ROOT, "%.1f", item.weight()));
        lines.add("格子: " + item.gridWidth() + "x" + item.gridHeight());
        lines.add("保质期: " + percent(item.durability()));
        lines.add("充能次数: " + chargeText(item));
        if (!item.description().isBlank()) {
            lines.add("描述: " + item.description());
        }
        if (isSpiritualMaterial(item)) {
            lines.add("灵材: 产地待鉴定 / 适配丹方看炼丹面板");
        }
        if (item.forgeQuality() != null || item.forgeAchievedTier() != null) {
            lines.add("法器: 灵核 T" + (item.forgeAchievedTier() == null ? "?" : item.forgeAchievedTier()));
            lines.add("铭文槽: " + (item.hasInscription() ? item.inscriptionId() : "空"));
            lines.add("当前附着: " + (item.forgeSideEffects().isEmpty() ? "无" : String.join(", ", item.forgeSideEffects())));
        }
        if (!item.alchemyLines().isEmpty()) {
            lines.addAll(item.alchemyLines());
        }
        return List.copyOf(lines);
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        renderBackground(context);
        int x = Math.max(12, (width - PANEL_WIDTH) / 2);
        int y = Math.max(12, height / 2 - 112);
        context.fill(x, y, x + PANEL_WIDTH, y + 224, BG);
        drawItemIcon(context, x + (PANEL_WIDTH - ICON_SIZE) / 2, y + 16);

        MinecraftClient client = MinecraftClient.getInstance();
        int lineY = y + 112;
        for (String line : detailLines(item)) {
            int color = line.startsWith("名称") ? TEXT_COLOR : MUTED_COLOR;
            context.drawTextWithShadow(client.textRenderer, line, x + 16, lineY, color);
            lineY += 11;
        }
        super.render(context, mouseX, mouseY, delta);
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    private void drawItemIcon(DrawContext context, int x, int y) {
        Identifier texture = GridSlotComponent.textureIdForItem(item);
        context.drawTexture(texture, x, y, 0.0f, 0.0f, ICON_SIZE, ICON_SIZE, 128, 128);
    }

    private static String percent(double ratio) {
        return Math.round(Math.max(0.0, Math.min(1.0, ratio)) * 100.0) + "%";
    }

    private static String chargeText(InventoryItem item) {
        if (item.forgeAchievedTier() != null) {
            return String.valueOf(Math.max(1, item.forgeAchievedTier()));
        }
        return "-";
    }

    private static boolean isSpiritualMaterial(InventoryItem item) {
        String id = item.itemId();
        return id.endsWith("_cao")
            || id.endsWith("_zhi")
            || id.endsWith("_guo")
            || id.endsWith("_teng")
            || id.endsWith("_gen")
            || id.endsWith("_hua")
            || item.alchemyLines().stream().anyMatch(line -> line.contains("丹") || line.contains("药"));
    }
}
