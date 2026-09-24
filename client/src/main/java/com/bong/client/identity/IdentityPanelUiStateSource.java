package com.bong.client.identity;

import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.contract.UiSubscriptions;
import com.bong.client.ui.state.StoreUiStateSource;

import java.util.Objects;
import java.util.function.Consumer;

/** 把身份 Store 的快照/监听器转换为屏幕可消费的状态源。 */
public final class IdentityPanelUiStateSource implements UiStateSource<IdentityPanelState> {
    private final UiStateSource<IdentityPanelState> delegate;

    private IdentityPanelUiStateSource(UiStateSource<IdentityPanelState> delegate) {
        this.delegate = Objects.requireNonNull(delegate, "delegate must not be null");
    }

    public static IdentityPanelUiStateSource production() {
        return new IdentityPanelUiStateSource(StoreUiStateSource.push(
            IdentityPanelStateStore::snapshot,
            listener -> {
                Consumer<IdentityPanelState> adapter = listener::accept;
                IdentityPanelStateStore.addListener(adapter);
                return UiSubscriptions.once(() -> IdentityPanelStateStore.removeListener(adapter));
            }
        ));
    }

    @Override
    public IdentityPanelState snapshot() {
        return delegate.snapshot();
    }

    @Override
    public UiSubscription subscribe(Consumer<? super IdentityPanelState> listener) {
        return delegate.subscribe(listener);
    }
}
