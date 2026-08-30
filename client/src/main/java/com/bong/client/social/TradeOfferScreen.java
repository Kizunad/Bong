package com.bong.client.social;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.ui.contract.DefaultUiScreenScope;
import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.intent.UiIntentResult;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

import java.util.List;
import java.util.Objects;
import java.util.Optional;

/** plan-social-v1 §6.2: minimal trade response prompt. */
public final class TradeOfferScreen extends Screen {
    private static final int BG_COLOR = 0xD0101218;
    private static final int PANEL_COLOR = 0xE0222630;
    private static final int TITLE_COLOR = 0xFFE9D9A6;
    private static final int TEXT_COLOR = 0xFFE8E8E8;
    private static final int MUTED_COLOR = 0xFF9AA4B2;
    private static final int WARNING_COLOR = 0xFFFFAA55;
    private static final int MAX_VISIBLE_ITEMS = 5;
    private static final long REQUEST_PICKER_TIMEOUT_MS = 30_000L;

    private enum Mode {
        RESPONSE,
        REQUEST
    }

    private final TradeOfferScreenController controller;
    private final DefaultUiScreenScope scope = new DefaultUiScreenScope();
    private final TradeOfferPicker picker;
    private final Mode mode;
    private final String requestTarget;
    private boolean settled;

    public TradeOfferScreen(SocialStateStore.TradeOffer offer) {
        this(offer, Mode.RESPONSE, null);
    }

    /** 打开发起交易的显式 picker，目标在打开时捕获，不依赖 picker 期间的准星。 */
    public static TradeOfferScreen requestPicker(String target) {
        String normalizedTarget = Objects.requireNonNull(target, "trade target must not be null").strip();
        if (normalizedTarget.isBlank()) throw new IllegalArgumentException("trade target must not be blank");
        SocialStateStore.TradeOffer context = new SocialStateStore.TradeOffer(
            "request:" + normalizedTarget,
            "",
            normalizedTarget,
            new SocialStateStore.TradeItemSummary(0L, "", "", 1),
            List.of(),
            System.currentTimeMillis() + REQUEST_PICKER_TIMEOUT_MS
        );
        return new TradeOfferScreen(context, Mode.REQUEST, normalizedTarget);
    }

    private TradeOfferScreen(SocialStateStore.TradeOffer offer, Mode mode, String requestTarget) {
        super(Text.literal("交易邀请"));
        this.controller = TradeOfferScreenController.production(offer, this::applyViewModel, TradeOfferScreen::executeOnClientThread);
        this.picker = new TradeOfferPicker(controller.viewModel().choices());
        this.mode = Objects.requireNonNull(mode, "mode must not be null");
        this.requestTarget = requestTarget;
    }

    @Override
    protected void init() {
        super.init();
        if (!scope.isOpen()) {
            scope.onOpen();
            controller.onOpen(scope);
        }
        int cx = width / 2;
        int y = height / 2 + 76;
        this.addDrawableChild(ButtonWidget.builder(Text.literal("上一件"), b -> moveSelection(-1))
            .dimensions(cx - 156, y, 72, 20)
            .build());
        this.addDrawableChild(ButtonWidget.builder(Text.literal(mode == Mode.REQUEST ? "发起交易" : "交换"), b -> settle(true))
            .dimensions(cx - 36, y, 72, 20)
            .build());
        this.addDrawableChild(ButtonWidget.builder(Text.literal(mode == Mode.REQUEST ? "取消" : "拒绝"), b -> settle(false))
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
        closeStateScope();
        super.close();
    }

    /**
     * Minecraft 在直接切换到另一屏幕时只调用 removed()；这里也必须解绑
     * source/controller，避免交易屏被 setScreen(null) 后继续监听库存。
     */
    @Override
    public void removed() {
        closeStateScope();
        super.removed();
    }

    private void closeStateScope() {
        if (scope != null && !scope.isClosed()) scope.close();
        controller.onClose();
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
        TradeOfferScreenViewModel model = controller.viewModel();
        List<InventoryItem> choices = model.choices();
        context.fill(0, 0, width, height, BG_COLOR);
        int panelW = Math.min(420, Math.max(320, width - 40));
        int panelH = 230;
        int panelX = (width - panelW) / 2;
        int panelY = (height - panelH) / 2;
        context.fill(panelX, panelY, panelX + panelW, panelY + panelH, PANEL_COLOR);
        String title = mode == Mode.REQUEST ? "◇ 发 起 交 易 ◇" : "◇ 交 易 邀 请 ◇";
        context.drawCenteredTextWithShadow(textRenderer, title, width / 2, panelY + 12, TITLE_COLOR);
        if (mode == Mode.REQUEST) {
            context.drawCenteredTextWithShadow(textRenderer, "目标: " + requestTarget, width / 2, panelY + 34, TEXT_COLOR);
            context.drawCenteredTextWithShadow(textRenderer, "选择你要提供的物品", width / 2, panelY + 50, MUTED_COLOR);
        } else {
            context.drawCenteredTextWithShadow(textRenderer, "对方提供: " + itemLabel(model.offer().offeredItem()), width / 2, panelY + 34, TEXT_COLOR);
            context.drawCenteredTextWithShadow(textRenderer, "倒计时: " + Math.max(0L, remainingMillis() / 1000L) + "s", width / 2, panelY + 50, WARNING_COLOR);
        }

        int y = panelY + 74;
        int selectedIndex = picker.selectedIndex();
        choices = picker.choices();
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
        picker.move(delta);
    }

    private void settle(boolean accepted) {
        if (settled) return;
        UiIntentResult result;
        if (mode == Mode.REQUEST) {
            if (!accepted) {
                settled = true;
                finishScreen();
                return;
            }
            controller.refreshFromSource();
            Optional<InventoryItem> selected = picker.selectedFrom(controller.viewModel().choices());
            if (selected.isEmpty()) return;
            settled = true;
            result = controller.intentSink().dispatch(new TradeOfferIntent.Request(
                requestTarget, selected.get().instanceId()
            ));
        } else {
            settled = true;
            List<InventoryItem> choices = controller.viewModel().choices();
            Long requested = accepted
                ? picker.selectedFrom(choices).map(InventoryItem::instanceId).orElse(null)
                : null;
            result = controller.intentSink().dispatch(new TradeOfferIntent.Respond(
                controller.viewModel().offer().offerId(), accepted, requested
            ));
        }
        if (result.kind() == UiIntentResult.Kind.LOCAL_REJECTED || result.kind() == UiIntentResult.Kind.LOCAL_ERROR) {
            settled = false;
            return;
        }
        finishScreen();
    }

    private void finishScreen() {
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc != null && mc.currentScreen == this) {
            mc.setScreen(null);
        } else {
            // 无客户端实例时（例如 headless 测试）也要完成本地生命周期清理。
            closeStateScope();
        }
    }

    private long remainingMillis() {
        return Math.max(0L, controller.viewModel().offer().expiresAtMs() - System.currentTimeMillis());
    }

    public String offerIdForTests() {
        return controller.viewModel().offer().offerId();
    }

    /** Bootstrap 用于区分出站 picker 与入站邀请屏，避免误清理出站交互。 */
    boolean isRequestPicker() {
        return mode == Mode.REQUEST;
    }

    /** 测试用：按 authoritative instance_id 查找当前 picker 的位置。 */
    static int selectionIndexForTests(List<InventoryItem> choices, long instanceId) {
        return TradeOfferPicker.indexOf(choices, instanceId);
    }

    private void applyViewModel(TradeOfferScreenViewModel model) {
        picker.update(model.choices());
    }

    private static void executeOnClientThread(Runnable action) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) action.run();
        else client.execute(action);
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
