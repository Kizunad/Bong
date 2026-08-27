package com.bong.client.ui.state;

import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;

import java.util.Objects;
import java.util.function.Consumer;

/**
 * 把一个状态源绑定到屏幕生命周期。首帧先读 snapshot，再订阅后续变化；
 * scope 关闭后，已排队的旧 Store 回调也不能再进入渲染层。
 */
public final class UiStateBinder {
    private UiStateBinder() {
    }

    public static <S> UiSubscription bind(
        UiStateSource<S> source,
        UiScreenScope scope,
        Consumer<? super S> listener
    ) {
        Objects.requireNonNull(source, "source must not be null");
        Objects.requireNonNull(scope, "scope must not be null");
        Objects.requireNonNull(listener, "listener must not be null");

        boolean initialDelivered = scope.runIfOpen(() -> listener.accept(source.snapshot()));
        if (!initialDelivered) {
            throw new IllegalStateException("cannot bind state to a scope that is not open");
        }

        UiSubscription subscription = source.subscribe(
            value -> scope.runIfOpen(() -> listener.accept(value))
        );
        try {
            scope.addCleanup(subscription::close);
        } catch (Throwable failure) {
            closeAfterRegistrationFailure(subscription, failure);
        }
        return subscription;
    }

    private static void closeAfterRegistrationFailure(
        UiSubscription subscription,
        Throwable registrationFailure
    ) {
        try {
            subscription.close();
        } catch (Throwable closeFailure) {
            if (registrationFailure != closeFailure) {
                registrationFailure.addSuppressed(closeFailure);
            }
        }
        throwUnchecked(registrationFailure);
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
