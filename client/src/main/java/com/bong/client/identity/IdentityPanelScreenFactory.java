package com.bong.client.identity;

import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/** 身份面板工厂；生产网络与 Store 适配器只在组合根组装。 */
public final class IdentityPanelScreenFactory {
    private final UiStateSource<IdentityPanelState> stateSource;
    private final UiIntentSink<IdentityPanelIntent> intentSink;

    public IdentityPanelScreenFactory(
        UiStateSource<IdentityPanelState> stateSource,
        UiIntentSink<IdentityPanelIntent> intentSink
    ) {
        this.stateSource = Objects.requireNonNull(stateSource, "stateSource must not be null");
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    public IdentityPanelScreen create() {
        return new IdentityPanelScreen(stateSource, intentSink);
    }
}
