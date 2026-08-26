package com.bong.client.ui.contract;

import java.util.Objects;

/** 创建恰好执行一次清理的订阅句柄。 */
public final class UiSubscriptions {
    private UiSubscriptions() {
    }

    public static UiSubscription once(Runnable closer) {
        return new OnceSubscription(Objects.requireNonNull(closer, "closer must not be null"));
    }

    public static UiSubscription closed() {
        UiSubscription subscription = once(() -> {
        });
        subscription.close();
        return subscription;
    }

    private static final class OnceSubscription implements UiSubscription {
        private final Runnable closer;
        private boolean closed;

        private OnceSubscription(Runnable closer) {
            this.closer = closer;
        }

        @Override
        public synchronized void close() {
            if (closed) {
                return;
            }
            closed = true;
            closer.run();
        }

        @Override
        public synchronized boolean isClosed() {
            return closed;
        }
    }
}
