package com.bong.client.inventory;

import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.contract.UiSubscriptions;
import com.bong.client.ui.state.StoreUiStateSource;

import java.util.Objects;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;

/** 将搜刮会话与玩家库存 listener 归一成一个 screen source。 */
public final class LootContainerUiStateSource implements UiStateSource<LootContainerScreenViewModel> {
    private final UiStateSource<LootContainerSession> session;
    private final UiStateSource<InventoryModel> inventory;
    private final AtomicLong revision = new AtomicLong();

    private LootContainerUiStateSource(
        UiStateSource<LootContainerSession> session,
        UiStateSource<InventoryModel> inventory
    ) {
        this.session = Objects.requireNonNull(session, "session source must not be null");
        this.inventory = Objects.requireNonNull(inventory, "inventory source must not be null");
    }

    public static LootContainerUiStateSource production(LootContainerSession.Open expected) {
        Objects.requireNonNull(expected, "expected session must not be null");
        // current 在关闭时会变为 null，但关闭事件本身仍是 authoritative 状态；
        // 用最后一次事件保留 Closed，避免 source 把已关闭会话误读回 expected。
        AtomicReference<LootContainerSession> latest = new AtomicReference<>(expected);
        return new LootContainerUiStateSource(
            StoreUiStateSource.push(
                () -> {
                    LootContainerStateStore.Session current = LootContainerStateStore.current();
                    if (current instanceof LootContainerStateStore.OpenSession open
                        && open.sessionId() == expected.sessionId()) {
                        latest.set(LootContainerSessionAdapter.open(open));
                    } else if (current instanceof LootContainerStateStore.Closed closed
                        && closed.sessionId() == expected.sessionId()) {
                        latest.set(new LootContainerSession.Closed(closed.sessionId(), closed.reason()));
                    } else if (current instanceof LootContainerStateStore.OpenSession) {
                        // 首次读取就遇到另一会话时直接 fail closed，不能把新会话投影到旧屏幕。
                        latest.set(new LootContainerSession.Closed(expected.sessionId(), "session replaced"));
                    } else if (current == null && latest.get() instanceof LootContainerSession.Open) {
                        // 断线清理不会补发 close 事件；读到空 current 时也不能让旧屏幕复活。
                        latest.set(new LootContainerSession.Closed(expected.sessionId(), "session unavailable"));
                    }
                    return latest.get();
                },
                listener -> {
                    LootContainerStateStore.Listener adapter = value -> {
                        LootContainerSession next = LootContainerSessionAdapter.from(value);
                        latest.set(next);
                        listener.accept(next);
                    };
                    LootContainerStateStore.addListener(adapter);
                    return UiSubscriptions.once(() -> LootContainerStateStore.removeListener(adapter));
                }
            ),
            StoreUiStateSource.push(
                InventoryStateStore::snapshot,
                listener -> {
                    Consumer<InventoryModel> adapter = listener::accept;
                    InventoryStateStore.addListener(adapter);
                    return UiSubscriptions.once(() -> InventoryStateStore.removeListener(adapter));
                }
            )
        );
    }

    public static LootContainerUiStateSource production(LootContainerStateStore.OpenSession expected) {
        return production(LootContainerSessionAdapter.open(expected));
    }

    @Override
    public LootContainerScreenViewModel snapshot() {
        return new LootContainerScreenViewModel(revision.get(), session.snapshot(), inventory.snapshot());
    }

    @Override
    public UiSubscription subscribe(Consumer<? super LootContainerScreenViewModel> listener) {
        Objects.requireNonNull(listener, "listener must not be null");
        UiSubscription sessionSubscription = session.subscribe(value -> listener.accept(next(value)));
        try {
            UiSubscription inventorySubscription = inventory.subscribe(ignored -> listener.accept(next(session.snapshot())));
            return UiSubscriptions.combine(sessionSubscription, inventorySubscription);
        } catch (Throwable failure) {
            try {
                sessionSubscription.close();
            } catch (Throwable closeFailure) {
                if (closeFailure != failure) failure.addSuppressed(closeFailure);
            }
            LootContainerUiStateSource.<RuntimeException>throwUnchecked(failure);
            throw new AssertionError("unreachable");
        }
    }

    private LootContainerScreenViewModel next(LootContainerSession sessionState) {
        return new LootContainerScreenViewModel(revision.incrementAndGet(), sessionState, inventory.snapshot());
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
