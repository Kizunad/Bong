package com.bong.client.ui.contract;

import com.bong.client.ui.intent.UiIntentSink;

/** 由 owo 与 vanilla 宿主共同消费的库无关控制器契约。 */
public interface UiScreenController<M, I extends UiIntent> {
    M viewModel();

    UiIntentSink<I> intentSink();

    void onOpen(UiScreenScope scope);

    void onClose();
}
