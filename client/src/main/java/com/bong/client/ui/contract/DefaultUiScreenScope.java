package com.bong.client.ui.contract;

import java.util.ArrayDeque;
import java.util.Deque;
import java.util.Objects;

/**
 * 适配器使用的默认生命周期域。关闭时先标记已关闭，再按后进先出顺序执行全部
 * 清理，避免某个失败回调让后续清理永久遗失。
 */
public final class DefaultUiScreenScope implements UiScreenScope {
    private final Object lock = new Object();
    private final Deque<Runnable> cleanups = new ArrayDeque<>();
    private boolean opened;
    private boolean closed;
    private long lastTickMs = -1L;

    @Override
    public void onOpen() {
        synchronized (lock) {
            if (closed) {
                throw new IllegalStateException("cannot open a closed UI scope");
            }
            opened = true;
        }
    }

    @Override
    public void addCleanup(Runnable cleanup) {
        Objects.requireNonNull(cleanup, "cleanup must not be null");
        synchronized (lock) {
            if (closed) {
                throw new IllegalStateException("cannot register cleanup after scope close");
            }
            cleanups.push(cleanup);
        }
    }

    @Override
    public void onTick(long nowMs) {
        synchronized (lock) {
            if (!closed && opened && nowMs >= lastTickMs) {
                lastTickMs = nowMs;
            }
        }
    }

    @Override
    public boolean runIfOpen(Runnable task) {
        Objects.requireNonNull(task, "task must not be null");
        synchronized (lock) {
            if (closed || !opened) {
                return false;
            }
            task.run();
            return true;
        }
    }

    @Override
    public void close() {
        synchronized (lock) {
            if (closed) {
                return;
            }
            closed = true;
        }

        Throwable primary = null;
        while (true) {
            Runnable cleanup;
            synchronized (lock) {
                cleanup = cleanups.pollFirst();
            }
            if (cleanup == null) {
                break;
            }
            try {
                cleanup.run();
            } catch (Throwable failure) {
                primary = appendFailure(primary, failure);
            }
        }
        if (primary != null) {
            throwUnchecked(primary);
        }
    }

    @Override
    public boolean isClosed() {
        synchronized (lock) {
            return closed;
        }
    }

    public boolean isOpen() {
        synchronized (lock) {
            return opened && !closed;
        }
    }

    public long lastTickMs() {
        synchronized (lock) {
            return lastTickMs;
        }
    }

    private static Throwable appendFailure(Throwable primary, Throwable failure) {
        if (primary == null) {
            return failure;
        }
        if (primary != failure) {
            primary.addSuppressed(failure);
        }
        return primary;
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
