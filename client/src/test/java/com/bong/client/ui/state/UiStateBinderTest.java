package com.bong.client.ui.state;

import com.bong.client.ui.contract.DefaultUiScreenScope;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.contract.UiSubscription;
import com.bong.client.ui.contract.UiSubscriptions;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executor;
import java.util.function.Consumer;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UiStateBinderTest {
    @Test
    void queuedExecutorOwnsEveryListenerCallback() {
        MutableSource source = new MutableSource("initial");
        DefaultUiScreenScope scope = openScope();
        List<Runnable> queued = new ArrayList<>();
        List<String> rendered = new ArrayList<>();

        UiStateBinder.bind(source, scope, rendered::add, queued::add);
        source.emit("next");

        assertTrue(rendered.isEmpty(), "状态源回调不得绕过注入的 UI executor");
        assertEquals(1, queued.size(), "连续更新应共用一个有序 drain task");

        queued.remove(0).run();

        assertEquals(List.of("initial", "next"), rendered,
            "executor drain 必须按 snapshot 后续变化顺序交付状态");
    }

    @Test
    void updateDuringSubscriptionIsNotLostOrDuplicatedByCatchUpRead() {
        MutableSource source = new MutableSource("initial", "during-subscribe");
        DefaultUiScreenScope scope = openScope();
        List<String> rendered = new ArrayList<>();

        UiStateBinder.bind(source, scope, rendered::add, Runnable::run);

        assertEquals(List.of("initial", "during-subscribe"), rendered,
            "订阅窗口内更新必须补发且不能重复为 catch-up 帧");
    }

    @Test
    void closingScopeDropsAlreadyQueuedUpdates() {
        MutableSource source = new MutableSource("initial");
        DefaultUiScreenScope scope = openScope();
        List<Runnable> queued = new ArrayList<>();
        List<String> rendered = new ArrayList<>();

        UiStateBinder.bind(source, scope, rendered::add, queued::add);
        scope.close();
        queued.remove(0).run();

        assertTrue(rendered.isEmpty(), "scope 关闭后已排队的旧 UI task 不得再进入 listener");
    }

    private static DefaultUiScreenScope openScope() {
        DefaultUiScreenScope scope = new DefaultUiScreenScope();
        scope.onOpen();
        return scope;
    }

    private static final class MutableSource implements UiStateSource<String> {
        private final List<Consumer<? super String>> listeners = new ArrayList<>();
        private final String emitDuringSubscribe;
        private String current;

        private MutableSource(String current) {
            this(current, null);
        }

        private MutableSource(String current, String emitDuringSubscribe) {
            this.current = current;
            this.emitDuringSubscribe = emitDuringSubscribe;
        }

        @Override
        public String snapshot() {
            return current;
        }

        @Override
        public UiSubscription subscribe(Consumer<? super String> listener) {
            listeners.add(listener);
            if (emitDuringSubscribe != null) {
                current = emitDuringSubscribe;
                listener.accept(current);
            }
            return UiSubscriptions.once(() -> listeners.remove(listener));
        }

        private void emit(String next) {
            current = next;
            List.copyOf(listeners).forEach(listener -> listener.accept(next));
        }
    }
}
