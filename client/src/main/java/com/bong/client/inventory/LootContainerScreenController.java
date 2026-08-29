package com.bong.client.inventory;

import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.ui.contract.UiScreenController;
import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.intent.UiIntentSink;
import com.bong.client.ui.state.UiStateBinder;

import java.util.Objects;
import java.util.concurrent.Executor;
import java.util.function.Consumer;

/** 搜刮屏 controller：状态与动作均经过 library-neutral 边界。 */
public final class LootContainerScreenController
    implements UiScreenController<LootContainerScreenViewModel, LootContainerIntent> {
    private final UiStateSource<LootContainerScreenViewModel> source;
    private final UiIntentSink<LootContainerIntent> sink;
    private final Consumer<? super LootContainerScreenViewModel> listener;
    private final Executor executor;
    private final UiIntentSink<LootContainerIntent> guardedSink = this::dispatchIfOpen;
    private LootContainerScreenViewModel viewModel;
    private UiScreenScope activeScope;
    private boolean opened;
    private boolean closed;

    public LootContainerScreenController(
        UiStateSource<LootContainerScreenViewModel> source,
        UiIntentSink<LootContainerIntent> sink,
        Consumer<? super LootContainerScreenViewModel> listener,
        Executor executor
    ) {
        this.source = Objects.requireNonNull(source, "source must not be null");
        this.sink = Objects.requireNonNull(sink, "sink must not be null");
        this.listener = Objects.requireNonNull(listener, "listener must not be null");
        this.executor = Objects.requireNonNull(executor, "executor must not be null");
        this.viewModel = source.snapshot();
    }

    public static LootContainerScreenController production(
        LootContainerStateStore.OpenSession session,
        Consumer<? super LootContainerScreenViewModel> listener,
        Executor executor
    ) {
        return new LootContainerScreenController(
            LootContainerUiStateSource.production(session),
            LootContainerClientIntentSink.production(),
            listener,
            executor
        );
    }

    @Override public LootContainerScreenViewModel viewModel() { return viewModel; }
    @Override public UiIntentSink<LootContainerIntent> intentSink() { return guardedSink; }

    @Override
    public void onOpen(UiScreenScope scope) {
        Objects.requireNonNull(scope, "scope must not be null");
        if (closed) throw new IllegalStateException("cannot reopen a closed loot controller");
        if (opened) return;
        opened = true;
        activeScope = scope;
        try {
            UiStateBinder.bind(source, scope, this::accept, executor);
        } catch (Throwable failure) {
            opened = false;
            activeScope = null;
            LootContainerScreenController.<RuntimeException>throwUnchecked(failure);
        }
    }

    public void refreshFromSource() {
        if (!closed) accept(source.snapshot());
    }

    @Override public void onClose() { closed = true; activeScope = null; }

    private com.bong.client.ui.intent.UiIntentResult dispatchIfOpen(LootContainerIntent intent) {
        if (closed || !opened || activeScope == null || activeScope.isClosed()) {
            return com.bong.client.ui.intent.UiIntentResult.rejected("loot screen is closed");
        }
        if (!(viewModel.session() instanceof LootContainerStateStore.OpenSession open)) {
            return com.bong.client.ui.intent.UiIntentResult.rejected("loot session is closed");
        }
        long intentSessionId = intent instanceof LootContainerIntent.Move move
            ? move.sessionId()
            : intent instanceof LootContainerIntent.Close close ? close.sessionId() : -1L;
        if (open.sessionId() != intentSessionId) {
            return com.bong.client.ui.intent.UiIntentResult.rejected("loot session is stale");
        }
        return sink.dispatch(intent);
    }

    private void accept(LootContainerScreenViewModel next) {
        if (closed) return;
        viewModel = Objects.requireNonNull(next, "view model must not be null");
        listener.accept(next);
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
