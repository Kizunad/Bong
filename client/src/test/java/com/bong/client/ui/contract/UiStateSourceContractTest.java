package com.bong.client.ui.contract;

import com.bong.client.ui.state.StoreUiStateSource;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UiStateSourceContractTest {
    @Test
    void subscriptionCloseIsExactlyOnceAndBlocksLateLegacyCallbacks() {
        List<java.util.function.Consumer<? super Integer>> listeners = new ArrayList<>();
        AtomicInteger delegateCloseCount = new AtomicInteger();
        AtomicInteger received = new AtomicInteger();
        StoreUiStateSource<Integer> source = StoreUiStateSource.push(
            () -> 7,
            listener -> {
                listeners.add(listener);
                return UiSubscriptions.once(delegateCloseCount::incrementAndGet);
            }
        );

        assertEquals(7, source.snapshot());
        UiSubscription subscription = source.subscribe(received::set);
        listeners.get(0).accept(8);
        assertEquals(8, received.get(), "关闭前的 listener 变化必须可见");

        subscription.close();
        subscription.close();
        listeners.get(0).accept(9);
        assertTrue(subscription.isClosed());
        assertEquals(1, delegateCloseCount.get(), "重复 close 不能重复移除 Store listener");
        assertEquals(8, received.get(), "关闭后的 legacy 排队回调必须被丢弃");
    }

    @Test
    void pullSourceHasClosedSubscriptionAndRejectsNullSnapshot() {
        UiStateSource<String> source = StoreUiStateSource.pullOnOpen(() -> "state");
        UiSubscription subscription = source.subscribe(ignored -> {
        });
        assertTrue(subscription.isClosed(), "没有 listener 的 pull source 必须返回已关闭句柄");

        assertThrows(NullPointerException.class,
            () -> StoreUiStateSource.pullOnOpen(() -> null).snapshot());
    }

    @Test
    void scopeClosesLifoContinuesAfterFailuresAndRejectsLateWork() {
        DefaultUiScreenScope scope = new DefaultUiScreenScope();
        List<String> events = new ArrayList<>();
        RuntimeException first = new RuntimeException("third failure");
        RuntimeException second = new RuntimeException("second failure");
        scope.addCleanup(() -> events.add("first"));
        scope.addCleanup(() -> {
            events.add("second");
            throw second;
        });
        scope.addCleanup(() -> {
            events.add("third");
            throw first;
        });

        assertFalse(scope.isOpen(), "未 onOpen 的 scope 不应报告为 open");
        scope.onOpen();
        assertTrue(scope.isOpen(), "onOpen 后 scope 必须报告为 open");
        assertTrue(scope.runIfOpen(() -> events.add("open")));
        scope.onTick(42L);
        assertEquals(42L, scope.lastTickMs());

        RuntimeException thrown = assertThrows(RuntimeException.class, scope::close);
        assertEquals(first, thrown, "LIFO 第一失败必须作为 primary 异常抛出");
        assertEquals(List.of("open", "third", "second", "first"), events,
            "primary 失败不能阻断后续 cleanup");
        assertEquals(List.of(second), List.of(thrown.getSuppressed()),
            "后续失败必须按执行顺序挂到 suppressed");
        assertFalse(scope.runIfOpen(() -> events.add("late")));
        scope.close();
        assertTrue(scope.isClosed());
        assertFalse(scope.isOpen(), "close 后 scope 必须报告为 closed 且非 open");
        assertThrows(IllegalStateException.class, scope::onOpen);
        assertThrows(IllegalStateException.class, () -> scope.addCleanup(() -> {
        }));
    }

    @Test
    void subscriptionCloserMarksClosedBeforeThrowingAndDoesNotRetry() {
        AtomicInteger calls = new AtomicInteger();
        UiSubscription subscription = UiSubscriptions.once(() -> {
            calls.incrementAndGet();
            throw new IllegalStateException("close failure");
        });
        assertThrows(IllegalStateException.class, subscription::close);
        subscription.close();
        assertTrue(subscription.isClosed());
        assertEquals(1, calls.get());
    }
}
