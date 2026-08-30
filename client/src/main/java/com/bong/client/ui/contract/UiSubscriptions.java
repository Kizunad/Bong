package com.bong.client.ui.contract;

import java.util.Arrays;
import java.util.List;
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

    /**
     * 把多个订阅合并为一个句柄。关闭顺序与登记顺序相反；单个关闭失败不会
     * 阻断其余订阅清理，后续失败按执行顺序挂到首个异常上。
     */
    public static UiSubscription combine(UiSubscription... subscriptions) {
        Objects.requireNonNull(subscriptions, "subscriptions must not be null");
        List<UiSubscription> delegates = Arrays.stream(subscriptions)
            .map(subscription -> Objects.requireNonNull(subscription, "subscription must not be null"))
            .toList();
        return once(() -> closeAll(delegates));
    }

    private static void closeAll(List<UiSubscription> subscriptions) {
        Throwable primary = null;
        for (int index = subscriptions.size() - 1; index >= 0; index--) {
            try {
                subscriptions.get(index).close();
            } catch (Throwable failure) {
                if (primary == null) {
                    primary = failure;
                } else if (primary != failure) {
                    primary.addSuppressed(failure);
                }
            }
        }
        if (primary != null) {
            throwUnchecked(primary);
        }
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
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
