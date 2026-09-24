package com.bong.client.craft;

import com.bong.client.ui.contract.UiScreenController;
import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import com.bong.client.ui.state.UiStateBinder;

import java.util.Objects;
import java.util.concurrent.Executor;
import java.util.function.Consumer;

/** 手搓状态、生命周期和 typed intent 的库无关控制器。 */
public final class CraftScreenController
    implements UiScreenController<CraftScreenViewModel, CraftIntent> {

    private final UiStateSource<CraftScreenViewModel> source;
    private final UiIntentSink<CraftIntent> transportSink;
    private final Consumer<? super CraftScreenViewModel> viewListener;
    private final Executor uiExecutor;
    private final UiIntentSink<CraftIntent> guardedIntentSink = this::dispatchIfOpen;

    private CraftScreenViewModel viewModel;
    private UiScreenScope activeScope;
    private boolean opened;
    private boolean closed;

    public CraftScreenController(
        UiStateSource<CraftScreenViewModel> source,
        UiIntentSink<CraftIntent> transportSink,
        Consumer<? super CraftScreenViewModel> viewListener
    ) {
        this(source, transportSink, viewListener, Runnable::run);
    }

    public CraftScreenController(
        UiStateSource<CraftScreenViewModel> source,
        UiIntentSink<CraftIntent> transportSink,
        Consumer<? super CraftScreenViewModel> viewListener,
        Executor uiExecutor
    ) {
        this.source = Objects.requireNonNull(source, "source must not be null");
        this.transportSink = Objects.requireNonNull(transportSink, "transportSink must not be null");
        this.viewListener = Objects.requireNonNull(viewListener, "viewListener must not be null");
        this.uiExecutor = Objects.requireNonNull(uiExecutor, "uiExecutor must not be null");
        this.viewModel = source.snapshot();
    }

    public static CraftScreenController production(
        Consumer<? super CraftScreenViewModel> viewListener
    ) {
        return production(viewListener, Runnable::run);
    }

    public static CraftScreenController production(
        Consumer<? super CraftScreenViewModel> viewListener,
        Executor uiExecutor
    ) {
        return new CraftScreenController(
            CraftUiStateSource.production(),
            CraftClientIntentSink.production(),
            viewListener,
            uiExecutor
        );
    }

    @Override
    public CraftScreenViewModel viewModel() {
        return viewModel;
    }

    @Override
    public UiIntentSink<CraftIntent> intentSink() {
        return guardedIntentSink;
    }

    @Override
    public void onOpen(UiScreenScope scope) {
        Objects.requireNonNull(scope, "scope must not be null");
        if (closed) {
            throw new IllegalStateException("cannot reopen a closed craft controller");
        }
        if (opened) {
            return;
        }
        opened = true;
        activeScope = scope;
        try {
            UiStateBinder.bind(source, scope, this::acceptViewModel, uiExecutor);
        } catch (Throwable failure) {
            opened = false;
            activeScope = null;
            CraftScreenController.<RuntimeException>throwUnchecked(failure);
        }
    }

    @Override
    public void onClose() {
        closed = true;
        activeScope = null;
    }

    private void acceptViewModel(CraftScreenViewModel next) {
        if (closed) {
            return;
        }
        viewModel = Objects.requireNonNull(next, "view model update must not be null");
        viewListener.accept(next);
    }

    private UiIntentResult dispatchIfOpen(CraftIntent intent) {
        if (closed || activeScope == null || activeScope.isClosed()) {
            return UiIntentResult.rejected("craft screen is closed");
        }
        if (!opened) {
            return UiIntentResult.rejected("craft screen is not open");
        }
        return transportSink.dispatch(intent);
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
