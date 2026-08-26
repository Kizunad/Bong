package com.bong.client.ui.contract;

import java.util.function.Consumer;

/** UI 状态的库无关读取边界。 */
public interface UiStateSource<S> {
    S snapshot();

    UiSubscription subscribe(Consumer<? super S> listener);
}
