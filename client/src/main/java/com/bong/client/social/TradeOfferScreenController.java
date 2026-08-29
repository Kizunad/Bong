package com.bong.client.social;

import com.bong.client.ui.contract.UiScreenController;
import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import com.bong.client.ui.state.UiStateBinder;

import java.util.Objects;
import java.util.concurrent.Executor;
import java.util.function.Consumer;

/** 交易屏 controller，保证 selection 只来自当前 ViewModel 的 instance_id。 */
public final class TradeOfferScreenController
    implements UiScreenController<TradeOfferScreenViewModel, TradeOfferIntent> {
    private final UiStateSource<TradeOfferScreenViewModel> source;
    private final UiIntentSink<TradeOfferIntent> sink;
    private final Consumer<? super TradeOfferScreenViewModel> listener;
    private final Executor executor;
    private final UiIntentSink<TradeOfferIntent> guardedSink = this::dispatchIfOpen;
    private TradeOfferScreenViewModel viewModel;
    private UiScreenScope activeScope;
    private boolean opened;
    private boolean closed;

    public TradeOfferScreenController(
        UiStateSource<TradeOfferScreenViewModel> source,
        UiIntentSink<TradeOfferIntent> sink,
        Consumer<? super TradeOfferScreenViewModel> listener,
        Executor executor
    ) {
        this.source = Objects.requireNonNull(source, "source must not be null");
        this.sink = Objects.requireNonNull(sink, "sink must not be null");
        this.listener = Objects.requireNonNull(listener, "listener must not be null");
        this.executor = Objects.requireNonNull(executor, "executor must not be null");
        this.viewModel = source.snapshot();
    }

    public static TradeOfferScreenController production(
        SocialStateStore.TradeOffer offer,
        Consumer<? super TradeOfferScreenViewModel> listener,
        Executor executor
    ) {
        return new TradeOfferScreenController(
            TradeOfferUiStateSource.production(offer),
            TradeOfferClientIntentSink.production(),
            listener,
            executor
        );
    }

    @Override public TradeOfferScreenViewModel viewModel() { return viewModel; }
    @Override public UiIntentSink<TradeOfferIntent> intentSink() { return guardedSink; }

    @Override
    public void onOpen(UiScreenScope scope) {
        Objects.requireNonNull(scope, "scope must not be null");
        if (closed) throw new IllegalStateException("cannot reopen a closed trade controller");
        if (opened) return;
        opened = true;
        activeScope = scope;
        try {
            UiStateBinder.bind(source, scope, this::accept, executor);
        } catch (Throwable failure) {
            opened = false;
            activeScope = null;
            TradeOfferScreenController.<RuntimeException>throwUnchecked(failure);
        }
    }

    public void refreshFromSource() {
        if (!closed) accept(source.snapshot());
    }

    @Override public void onClose() { closed = true; activeScope = null; }

    private UiIntentResult dispatchIfOpen(TradeOfferIntent intent) {
        if (closed || !opened || activeScope == null || activeScope.isClosed()) {
            return UiIntentResult.rejected("trade screen is closed");
        }
        if (intent instanceof TradeOfferIntent.Respond response && response.accepted()) {
            long id = response.requestedInstanceId() == null ? -1L : response.requestedInstanceId();
            boolean present = viewModel.choices().stream().anyMatch(item -> item.instanceId() == id);
            if (!present) return UiIntentResult.rejected("selected item is no longer in the current inventory snapshot");
        }
        if (intent instanceof TradeOfferIntent.Respond response
            && !Objects.equals(response.offerId(), viewModel.offer().offerId())) {
            return UiIntentResult.rejected("trade offer is stale");
        }
        return sink.dispatch(intent);
    }

    private void accept(TradeOfferScreenViewModel next) {
        if (closed) return;
        viewModel = Objects.requireNonNull(next, "view model must not be null");
        listener.accept(next);
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
