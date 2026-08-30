package com.bong.client.alchemy;

import com.bong.client.ui.contract.UiScreenController;
import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.intent.UiIntentSink;
import com.bong.client.ui.state.UiStateBinder;

import java.util.Objects;
import java.util.concurrent.Executor;
import java.util.function.Consumer;

/** 炼丹屏 controller：只依赖 immutable ViewModel、source 和 typed sink。 */
public final class AlchemyScreenController
    implements UiScreenController<AlchemyScreenViewModel, AlchemyIntent> {
    private final UiStateSource<AlchemyScreenViewModel> source;
    private final UiIntentSink<AlchemyIntent> sink;
    private final Consumer<? super AlchemyScreenViewModel> listener;
    private final Executor executor;
    private AlchemyScreenViewModel viewModel;
    private UiScreenScope activeScope;
    private boolean opened;
    private boolean closed;
    private final UiIntentSink<AlchemyIntent> guardedSink = this::dispatchIfOpen;

    public AlchemyScreenController(
        UiStateSource<AlchemyScreenViewModel> source,
        UiIntentSink<AlchemyIntent> sink,
        Consumer<? super AlchemyScreenViewModel> listener,
        Executor executor
    ) {
        this.source = Objects.requireNonNull(source, "source must not be null");
        this.sink = Objects.requireNonNull(sink, "sink must not be null");
        this.listener = Objects.requireNonNull(listener, "listener must not be null");
        this.executor = Objects.requireNonNull(executor, "executor must not be null");
        this.viewModel = source.snapshot();
    }

    public static AlchemyScreenController production(
        Consumer<? super AlchemyScreenViewModel> listener,
        Executor executor
    ) {
        return new AlchemyScreenController(
            AlchemyUiStateSource.production(),
            AlchemyClientIntentSink.production(),
            listener,
            executor
        );
    }

    @Override public AlchemyScreenViewModel viewModel() { return viewModel; }

    @Override public UiIntentSink<AlchemyIntent> intentSink() { return guardedSink; }

    @Override
    public void onOpen(UiScreenScope scope) {
        Objects.requireNonNull(scope, "scope must not be null");
        if (closed) throw new IllegalStateException("cannot reopen a closed alchemy controller");
        if (opened) return;
        opened = true;
        activeScope = scope;
        try {
            UiStateBinder.bind(source, scope, this::accept, executor);
        } catch (Throwable failure) {
            opened = false;
            activeScope = null;
            AlchemyScreenController.<RuntimeException>throwUnchecked(failure);
        }
    }

    public void refreshFromSource() {
        if (!closed) accept(source.snapshot());
    }

    @Override
    public void onClose() {
        closed = true;
        activeScope = null;
    }

    private com.bong.client.ui.intent.UiIntentResult dispatchIfOpen(AlchemyIntent intent) {
        if (closed || !opened || activeScope == null || activeScope.isClosed()) {
            return com.bong.client.ui.intent.UiIntentResult.rejected("alchemy screen is closed");
        }
        return sink.dispatch(intent);
    }

    private void accept(AlchemyScreenViewModel next) {
        if (closed) return;
        viewModel = Objects.requireNonNull(next, "view model must not be null");
        listener.accept(next);
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
