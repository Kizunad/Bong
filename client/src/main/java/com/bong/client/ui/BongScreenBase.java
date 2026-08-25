package com.bong.client.ui;

import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.ParentComponent;
import net.minecraft.text.Text;
import org.jetbrains.annotations.NotNull;

import java.util.ArrayDeque;
import java.util.Deque;
import java.util.Objects;

/**
 * Shared lifecycle boundary for owo-backed Bong screens.
 *
 * <p>The base owns only state local to one screen instance.  Store/session
 * lifecycle remains outside this class; callers register the listener
 * removers that belong to this screen through {@link #registerCleanup(Runnable)}.
 * Queued callers should invoke {@link #runWhileOpen(Runnable)} when their task
 * reaches the client thread, so a task that was queued before removal is
 * discarded at its execution point.</p>
 *
 * @param <R> the root component supplied by the concrete screen's adapter
 */
public abstract class BongScreenBase<R extends ParentComponent> extends BaseOwoScreen<R> {
    private final Object lifecycleLock = new Object();
    private final Deque<Runnable> cleanups = new ArrayDeque<>();
    private volatile boolean closed;

    protected BongScreenBase() {
        super();
    }

    protected BongScreenBase(Text title) {
        super(Objects.requireNonNull(title, "title must not be null"));
    }

    @Override
    protected abstract @NotNull OwoUIAdapter<R> createAdapter();

    @Override
    protected abstract void build(R rootComponent);

    /**
     * Registers one screen-local unsubscriber or other teardown action.
     * Registrations are executed in reverse registration order.
     */
    protected final void registerCleanup(Runnable cleanup) {
        Objects.requireNonNull(cleanup, "cleanup must not be null");
        synchronized (lifecycleLock) {
            if (closed) {
                throw new IllegalStateException("cannot register cleanup after screen removal");
            }
            cleanups.push(cleanup);
        }
    }

    /**
     * Runs a refresh only if this screen is still open when the queued task
     * reaches this method.
     */
    protected final void runWhileOpen(Runnable task) {
        Objects.requireNonNull(task, "task must not be null");
        synchronized (lifecycleLock) {
            if (closed) {
                return;
            }
            task.run();
        }
    }

    @Override
    public final void tick() {
        synchronized (lifecycleLock) {
            if (closed) {
                return;
            }
            super.tick();
            if (!closed) {
                onScreenTick();
            }
        }
    }

    /** Hook for per-tick work while the screen is open. */
    protected void onScreenTick() {
    }

    @Override
    public final void removed() {
        synchronized (lifecycleLock) {
            if (closed) {
                return;
            }
            closed = true;
        }

        Throwable primary = null;
        try {
            onRemoved();
        } catch (Throwable failure) {
            primary = failure;
        }

        while (true) {
            Runnable cleanup;
            synchronized (lifecycleLock) {
                cleanup = cleanups.pollFirst();
            }
            if (cleanup == null) {
                break;
            }
            try {
                cleanup.run();
            } catch (Throwable failure) {
                primary = recordFailure(primary, failure);
            }
        }

        try {
            super.removed();
        } catch (Throwable failure) {
            primary = recordFailure(primary, failure);
        }

        if (primary != null) {
            throwUnchecked(primary);
        }
    }

    /** Hook for business terminal effects; cleanup and parent removal always follow it. */
    protected void onRemoved() {
    }

    private static Throwable recordFailure(Throwable primary, Throwable failure) {
        if (primary == null) {
            return failure;
        }
        if (primary != failure) {
            try {
                primary.addSuppressed(failure);
            } catch (Throwable ignored) {
                // A pathological Throwable (for example, suppression disabled)
                // must not prevent the remaining lifecycle stages from running.
            }
        }
        return primary;
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
