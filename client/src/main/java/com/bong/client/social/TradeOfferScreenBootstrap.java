package com.bong.client.social;

import com.bong.client.BongClient;
import com.bong.client.hud.BongToast;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

import java.util.Comparator;

public final class TradeOfferScreenBootstrap {
    // F4 fix — 交易邀请被其他 GUI 挡时 / 静默过期时的非阻塞提示色（沿用 BongToast.WARNING_COLOR 数值，
    // 该常量为 hud 包内 package-private，跨包无法直接引用）。
    private static final int TOAST_COLOR = 0xFFAA55;

    private static String lastBlockedToastOfferId = "";

    private TradeOfferScreenBootstrap() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(TradeOfferScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered incoming social trade screen tick; outgoing trade uses unified G key");
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        handleIncomingOffer(client);
    }

    /** Screen 状态分类 —— 与 MinecraftClient/Screen 解耦，使 {@link #decide} 可脱离渲染环境单测。 */
    enum ScreenKind {
        /** 没有屏幕打开。 */
        NONE,
        /** 当前打开的 TradeOfferScreen 就是这份 offer 对应的那一份。 */
        MATCHING_TRADE_OFFER,
        /** 当前打开的是一份陈旧的 TradeOfferScreen（对应另一个 offerId，需要换新）。 */
        OTHER_TRADE_OFFER,
        /** 当前打开的是别的（非交易）GUI。 */
        OTHER
    }

    /** F4 fix — 决策结果，副作用（网络请求/开关屏/toast）全部留给调用方在 {@link #handleIncomingOffer} 里执行。 */
    enum Decision {
        NOOP,
        CLOSE_SCREEN,
        DECLINE_EXPIRED,
        OPEN_SCREEN,
        BLOCKED_TOAST
    }

    /**
     * F4 fix — 纯决策函数：不碰 MinecraftClient/Screen，只看 offer + 屏幕分类 + 当前时间。
     * 分支语义与重构前的 {@code handleIncomingOffer} 完全一致，新增 {@code BLOCKED_TOAST}
     * 用来替换原来"其他屏打开时静默 return"的分支。
     */
    static Decision decide(SocialStateStore.TradeOffer offer, ScreenKind screenKind, long nowMs) {
        if (offer == null) {
            return screenKind == ScreenKind.MATCHING_TRADE_OFFER || screenKind == ScreenKind.OTHER_TRADE_OFFER
                ? Decision.CLOSE_SCREEN
                : Decision.NOOP;
        }
        if (offer.expiresAtMs() <= nowMs) {
            return Decision.DECLINE_EXPIRED;
        }
        if (screenKind == ScreenKind.MATCHING_TRADE_OFFER) {
            return Decision.NOOP;
        }
        if (screenKind == ScreenKind.OTHER) {
            return Decision.BLOCKED_TOAST;
        }
        // NONE（没有屏幕）或 OTHER_TRADE_OFFER（陈旧的交易屏）都需要换一份新的 TradeOfferScreen。
        return Decision.OPEN_SCREEN;
    }

    private static void handleIncomingOffer(MinecraftClient client) {
        SocialStateStore.TradeOffer offer = SocialStateStore.tradeOffer();
        Screen current = client.currentScreen;
        ScreenKind screenKind = screenKindOf(current, offer);
        Decision decision = decide(offer, screenKind, System.currentTimeMillis());
        switch (decision) {
            case CLOSE_SCREEN -> client.setScreen(null);
            case DECLINE_EXPIRED -> {
                ClientRequestSender.sendTradeOfferResponse(offer.offerId(), false, null);
                SocialStateStore.clearTradeOffer(offer.offerId());
                if (current instanceof TradeOfferScreen) {
                    client.setScreen(null);
                }
                notifyExpired();
                lastBlockedToastOfferId = "";
            }
            case OPEN_SCREEN -> {
                client.setScreen(new TradeOfferScreen(offer));
                lastBlockedToastOfferId = "";
            }
            case BLOCKED_TOAST -> notifyBlocked(offer.offerId());
            case NOOP -> {
            }
        }
    }

    private static ScreenKind screenKindOf(Screen current, SocialStateStore.TradeOffer offer) {
        if (current instanceof TradeOfferScreen screen) {
            return offer != null && screen.offerIdForTests().equals(offer.offerId())
                ? ScreenKind.MATCHING_TRADE_OFFER
                : ScreenKind.OTHER_TRADE_OFFER;
        }
        return current == null ? ScreenKind.NONE : ScreenKind.OTHER;
    }

    /**
     * F4 fix — 其他 GUI 挡住交易邀请时的一次性非阻塞提示，按 offerId 去重，
     * 避免每 tick 都重新弹一次（同一份 offer 在被挡期间只提示一次）。
     */
    static void notifyBlocked(String offerId) {
        if (offerId == null || offerId.isBlank() || offerId.equals(lastBlockedToastOfferId)) {
            return;
        }
        lastBlockedToastOfferId = offerId;
        BongToast.show("交易邀请到达 · 关闭当前界面查看", TOAST_COLOR, System.currentTimeMillis(), 4_000L);
    }

    /** F4 fix — offer 静默过期自动拒绝时的提示，此前玩家完全零感知。 */
    static void notifyExpired() {
        BongToast.show("交易邀请已过期", TOAST_COLOR, System.currentTimeMillis(), 3_000L);
    }

    static void resetForTests() {
        lastBlockedToastOfferId = "";
    }

    static InventoryItem firstTradeItem(InventoryModel model) {
        if (model == null) return null;
        return model.gridItems().stream()
            .map(InventoryModel.GridEntry::item)
            .filter(item -> item != null && !item.isEmpty() && item.instanceId() > 0)
            .min(Comparator.comparing(InventoryItem::displayName).thenComparingLong(InventoryItem::instanceId))
            .orElseGet(() -> model.hotbar().stream()
                .filter(item -> item != null && !item.isEmpty() && item.instanceId() > 0)
                .min(Comparator.comparing(InventoryItem::displayName).thenComparingLong(InventoryItem::instanceId))
                .orElse(null));
    }
}
