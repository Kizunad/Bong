package com.bong.client.social;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.contract.UiSubscriptions;
import com.bong.client.ui.state.StoreUiStateSource;

import java.util.Objects;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Consumer;

/** 组合交易 offer 与库存快照；offer 本身没有 listener，按屏幕打开时读取。 */
public final class TradeOfferUiStateSource implements UiStateSource<TradeOfferScreenViewModel> {
    private final UiStateSource<SocialStateStore.TradeOffer> offer;
    private final UiStateSource<InventoryModel> inventory;
    private final AtomicLong revision = new AtomicLong();

    private TradeOfferUiStateSource(
        UiStateSource<SocialStateStore.TradeOffer> offer,
        UiStateSource<InventoryModel> inventory
    ) {
        this.offer = Objects.requireNonNull(offer, "offer source must not be null");
        this.inventory = Objects.requireNonNull(inventory, "inventory source must not be null");
    }

    public static TradeOfferUiStateSource production(SocialStateStore.TradeOffer expectedOffer) {
        Objects.requireNonNull(expectedOffer, "expected offer must not be null");
        return new TradeOfferUiStateSource(
            // offer 是打开时的身份快照；后续只允许库存 source 推送 choices，
            // 不能因全局 Store 被替换而把当前屏幕悄悄切到另一份邀请。
            StoreUiStateSource.pullOnOpen(() -> expectedOffer),
            StoreUiStateSource.push(
                InventoryStateStore::snapshot,
                listener -> {
                    Consumer<InventoryModel> adapter = listener::accept;
                    InventoryStateStore.addListener(adapter);
                    return UiSubscriptions.once(() -> InventoryStateStore.removeListener(adapter));
                }
            )
        );
    }

    @Override
    public TradeOfferScreenViewModel snapshot() {
        SocialStateStore.TradeOffer current = offer.snapshot();
        return new TradeOfferScreenViewModel(
            revision.get(), current, TradeOfferScreenViewModel.collectChoices(inventory.snapshot())
        );
    }

    @Override
    public UiSubscription subscribe(Consumer<? super TradeOfferScreenViewModel> listener) {
        Objects.requireNonNull(listener, "listener must not be null");
        return inventory.subscribe(ignored -> listener.accept(snapshotWithNextRevision()));
    }

    private TradeOfferScreenViewModel snapshotWithNextRevision() {
        SocialStateStore.TradeOffer current = offer.snapshot();
        return new TradeOfferScreenViewModel(
            revision.incrementAndGet(), current, TradeOfferScreenViewModel.collectChoices(inventory.snapshot())
        );
    }
}
