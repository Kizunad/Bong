package com.bong.client.ui.state;

import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.contract.UiSubscriptions;

import java.util.Objects;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Supplier;

/**
 * 将现有 Store 的 snapshot/listener 对适配为 UI source，不把 Store 字段暴露给
 * Screen。即使旧 Store 在拆卸阶段补发一个排队回调，关闭后的句柄也会
 * 先拦截该回调。
 */
public final class StoreUiStateSource<S> implements UiStateSource<S> {
    private final Supplier<? extends S> snapshotReader;
    private final Function<Consumer<? super S>, ? extends UiSubscription> listenerRegistrar;

    private StoreUiStateSource(
        Supplier<? extends S> snapshotReader,
        Function<Consumer<? super S>, ? extends UiSubscription> listenerRegistrar
    ) {
        this.snapshotReader = Objects.requireNonNull(snapshotReader, "snapshotReader must not be null");
        this.listenerRegistrar = Objects.requireNonNull(listenerRegistrar, "listenerRegistrar must not be null");
    }

    public static <S> StoreUiStateSource<S> pullOnOpen(Supplier<? extends S> snapshotReader) {
        return new StoreUiStateSource<>(snapshotReader, ignored -> UiSubscriptions.closed());
    }

    public static <S> StoreUiStateSource<S> push(
        Supplier<? extends S> snapshotReader,
        Function<Consumer<? super S>, ? extends UiSubscription> listenerRegistrar
    ) {
        return new StoreUiStateSource<>(snapshotReader, listenerRegistrar);
    }

    @Override
    public S snapshot() {
        return Objects.requireNonNull(snapshotReader.get(), "Store snapshot must not be null");
    }

    @Override
    public UiSubscription subscribe(Consumer<? super S> listener) {
        Objects.requireNonNull(listener, "listener must not be null");
        AtomicBoolean closed = new AtomicBoolean();
        UiSubscription delegate = Objects.requireNonNull(
            listenerRegistrar.apply(value -> {
                if (!closed.get()) {
                    listener.accept(value);
                }
            }),
            "listener registrar must return a subscription"
        );
        UiSubscription guarded = UiSubscriptions.once(() -> {
            closed.set(true);
            delegate.close();
        });
        if (delegate.isClosed()) {
            guarded.close();
        }
        return guarded;
    }
}
