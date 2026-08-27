package com.bong.client.ui.adapter.owo;

import com.bong.client.ui.contract.DefaultUiScreenScope;
import com.bong.client.ui.contract.UiScreenScope;

import java.util.Objects;
import java.util.function.Consumer;

/** 统一协调 XML host 的一次打开、幂等关闭与 late callback 门禁。 */
final class OwoXmlHostLifecycle {
    private final DefaultUiScreenScope scope = new DefaultUiScreenScope();
    private boolean opened;
    private boolean hostClosed;

    void openOnce(Consumer<UiScreenScope> onOpened) {
        Objects.requireNonNull(onOpened, "onOpened must not be null");
        if (opened || scope.isClosed()) {
            return;
        }
        opened = true;
        scope.onOpen();
        onOpened.accept(scope);
    }

    void tick(long nowMs) {
        scope.onTick(nowMs);
    }

    void close(Runnable onHostClosed) {
        Objects.requireNonNull(onHostClosed, "onHostClosed must not be null");
        Throwable primary = null;
        try {
            scope.close();
        } catch (Throwable failure) {
            primary = failure;
        }
        if (!hostClosed) {
            hostClosed = true;
            try {
                onHostClosed.run();
            } catch (Throwable failure) {
                primary = appendFailure(primary, failure);
            }
        }
        throwIfPresent(primary);
    }

    UiScreenScope scope() {
        return scope;
    }

    boolean isClosed() {
        return scope.isClosed();
    }

    static Throwable appendFailure(Throwable primary, Throwable failure) {
        if (primary == null) {
            return failure;
        }
        if (primary != failure) {
            primary.addSuppressed(failure);
        }
        return primary;
    }

    static void throwIfPresent(Throwable failure) {
        if (failure != null) {
            OwoXmlHostLifecycle.<RuntimeException>throwUnchecked(failure);
        }
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
