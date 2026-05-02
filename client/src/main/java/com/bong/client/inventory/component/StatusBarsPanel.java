package com.bong.client.inventory.component;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.util.RealmLabel;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

public class StatusBarsPanel extends BaseComponent {
    private static final int PANEL_WIDTH = 140;
    private static final int PANEL_HEIGHT = 34;
    private static final int BAR_HEIGHT = 5;
    private static final int BAR_MARGIN = 2;
    private static final int TEXT_COLOR = 0xFFAAAAAA;
    private static final int QI_BAR_FULL = 0xFF5588BB;
    private static final int QI_BAR_EMPTY = 0xFF2A2A2A;
    private static final int BODY_BAR_FULL = 0xFFBB5555;
    private static final int BODY_BAR_EMPTY = 0xFF2A2A2A;

    private String realm = "";
    private double qiFillRatio = 0.0;
    private double bodyLevel = 0.0;

    public StatusBarsPanel() {
        this.sizing(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(PANEL_HEIGHT));
    }

    public void updateFromModel(InventoryModel model) {
        this.realm = model.realm();
        this.qiFillRatio = model.qiFillRatio();
        this.bodyLevel = Math.max(0.0, Math.min(1.0, model.bodyLevel()));
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        var textRenderer = MinecraftClient.getInstance().textRenderer;
        int cy = y;

        // Realm label
        context.drawTextWithShadow(textRenderer, Text.literal("境界: " + RealmLabel.displayName(realm)), x + 2, cy, TEXT_COLOR);
        cy += textRenderer.fontHeight + 2;

        // Qi bar
        context.drawTextWithShadow(textRenderer, Text.literal("真元"), x + 2, cy, TEXT_COLOR);
        int barX = x + 30;
        int barW = PANEL_WIDTH - 34;
        drawBar(context, barX, cy, barW, BAR_HEIGHT, qiFillRatio, QI_BAR_FULL, QI_BAR_EMPTY);
        cy += BAR_HEIGHT + BAR_MARGIN;

        // Body bar
        context.drawTextWithShadow(textRenderer, Text.literal("体魄"), x + 2, cy, TEXT_COLOR);
        drawBar(context, barX, cy, barW, BAR_HEIGHT, bodyLevel, BODY_BAR_FULL, BODY_BAR_EMPTY);
    }

    private static void drawBar(OwoUIDrawContext context, int bx, int by, int bw, int bh, double ratio, int fullColor, int emptyColor) {
        context.fill(bx, by, bx + bw, by + bh, emptyColor);
        int filled = (int) (bw * Math.max(0.0, Math.min(1.0, ratio)));
        if (filled > 0) {
            context.fill(bx, by, bx + filled, by + bh, fullColor);
        }
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return PANEL_WIDTH; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return PANEL_HEIGHT; }
}
