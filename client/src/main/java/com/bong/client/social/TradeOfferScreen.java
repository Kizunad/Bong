package com.bong.client.social;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/** plan-social-v1 §6.2: minimal trade response prompt. */
public final class TradeOfferScreen extends Screen {
    private static final int BG_COLOR = 0xD0101218;
    private static final int PANEL_COLOR = 0xE0222630;
    private static final int TITLE_COLOR = 0xFFE9D9A6;
    private static final int TEXT_COLOR = 0xFFE8E8E8;
    private static final int MUTED_COLOR = 0xFF9AA4B2;
    private static final int WARNING_COLOR = 0xFFFFAA55;
    private static final int MAX_VISIBLE_ITEMS = 5;

    private final SocialStateStore.TradeOffer offer;
    private final List<InventoryItem> choices;
    private int selectedIndex;
    private boolean settled;

    public TradeOfferScreen(SocialStateStore.TradeOffer offer) {
        super(Text.literal("交易邀请"));
        this.offer = offer;
        this.choices = collectTradeChoices(InventoryStateStore.snapshot());
        this.selectedIndex = choices.isEmpty() ? -1 : 0;
    }

    @Override
    protected void init() {
        super.init();
        int cx = width / 2;
        int y = height / 2 + 76;
        this.addDrawableChild(ButtonWidget.builder(Text.literal("上一件"), b -> moveSelection(-1))
            .dimensions(cx - 156, y, 72, 20)
            .build());
        this.addDrawableChild(ButtonWidget.builder(Text.literal("交换"), b -> settle(true))
            .dimensions(cx - 36, y, 72, 20)
            .build());
        this.addDrawableChild(ButtonWidget.builder(Text.literal("拒绝"), b -> settle(false))
            .dimensions(cx + 84, y, 72, 20)
            .build());
        this.addDrawableChild(ButtonWidget.builder(Text.literal("下一件"), b -> moveSelection(1))
            .dimensions(cx - 36, y + 24, 72, 20)
            .build());
    }

    @Override
    public void tick() {
        super.tick();
        if (!settled && remainingMillis() <= 0L) {
            settle(false);
        }
    }

    @Override
    public void close() {
        if (!settled) {
            settle(false);
            return;
        }
        super.close();
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    public boolean mouseScrolled(double mouseX, double mouseY, double horizontalAmount, double verticalAmount) {
        if (verticalAmount > 0) moveSelection(-1);
        if (verticalAmount < 0) moveSelection(1);
        return true;
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        int panelW = Math.min(420, Math.max(320, width - 40));
        int panelH = 230;
        int panelX = (width - panelW) / 2;
        int panelY = (height - panelH) / 2;
        context.fill(panelX, panelY, panelX + panelW, panelY + panelH, PANEL_COLOR);
        context.drawCenteredTextWithShadow(textRenderer, "◇ 交 易 邀 请 ◇", width / 2, panelY + 12, TITLE_COLOR);
        context.drawCenteredTextWithShadow(textRenderer, "对方提供: " + itemLabel(offer.offeredItem()), width / 2, panelY + 34, TEXT_COLOR);
        context.drawCenteredTextWithShadow(textRenderer, "倒计时: " + Math.max(0L, remainingMillis() / 1000L) + "s", width / 2, panelY + 50, WARNING_COLOR);

        int y = panelY + 74;
        if (choices.isEmpty()) {
            context.drawCenteredTextWithShadow(textRenderer, "你当前没有可交换物品", width / 2, y, MUTED_COLOR);
        } else {
            int start = Math.max(0, Math.min(selectedIndex - 2, Math.max(0, choices.size() - MAX_VISIBLE_ITEMS)));
            int end = Math.min(choices.size(), start + MAX_VISIBLE_ITEMS);
            for (int i = start; i < end; i++) {
                InventoryItem item = choices.get(i);
                int color = i == selectedIndex ? TITLE_COLOR : MUTED_COLOR;
                String marker = i == selectedIndex ? "> " : "  ";
                context.drawTextWithShadow(textRenderer, marker + itemLabel(item), panelX + 34, y, color);
                y += 16;
            }
        }
        super.render(context, mouseX, mouseY, delta);
    }

    private void moveSelection(int delta) {
        if (choices.isEmpty()) return;
        selectedIndex = Math.floorMod(selectedIndex + delta, choices.size());
    }

    private void settle(boolean accepted) {
        if (settled) return;
        settled = true;
        Long requested = accepted && selectedIndex >= 0 && selectedIndex < choices.size()
            ? choices.get(selectedIndex).instanceId()
            : null;
        ClientRequestSender.sendTradeOfferResponse(offer.offerId(), accepted && requested != null, requested);
        SocialStateStore.clearTradeOffer(offer.offerId());
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.currentScreen == this) {
            mc.setScreen(null);
        }
    }

    private long remainingMillis() {
        return Math.max(0L, offer.expiresAtMs() - System.currentTimeMillis());
    }

    public String offerIdForTests() {
        return offer.offerId();
    }

    private static List<InventoryItem> collectTradeChoices(InventoryModel model) {
        ArrayList<InventoryItem> items = new ArrayList<>();
        if (model == null) return List.of();
        for (InventoryModel.GridEntry entry : model.gridItems()) {
            InventoryItem item = entry.item();
            if (item != null && !item.isEmpty() && item.instanceId() > 0) items.add(item);
        }
        for (InventoryItem item : model.hotbar()) {
            if (item != null && !item.isEmpty() && item.instanceId() > 0) items.add(item);
        }
        items.sort(Comparator.comparing(InventoryItem::displayName).thenComparingLong(InventoryItem::instanceId));
        return List.copyOf(items);
    }

    private static String itemLabel(SocialStateStore.TradeItemSummary item) {
        if (item == null) return "未知物品";
        String count = item.stackCount() > 1 ? " x" + item.stackCount() : "";
        return fallback(item.displayName(), item.itemId()) + count;
    }

    private static String itemLabel(InventoryItem item) {
        String count = item.stackCount() > 1 ? " x" + item.stackCount() : "";
        return fallback(item.displayName(), item.itemId()) + count;
    }

    private static String fallback(String value, String fallback) {
        return value == null || value.isBlank() ? fallback : value;
    }
}
