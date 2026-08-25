package com.bong.client.ui;

import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.ParentComponent;
import net.minecraft.text.Text;
import org.jetbrains.annotations.NotNull;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BongScreenBaseTest {
    @Test
    void bothConstructorsKeepTheExpectedTitleAndNullTitleIsRejected() {
        assertEquals("", new Harness().getTitle().getString(),
            "无参构造应沿用 BaseOwoScreen 的空标题路径");
        assertEquals("测试标题", new Harness(Text.literal("测试标题")).getTitle().getString(),
            "标题构造应把调用方 Text 原样交给 BaseOwoScreen");
        assertThrows(NullPointerException.class, () -> new Harness(null),
            "标题构造不应接受 null 并把错误延迟到 owo 初始化");
    }

    @Test
    void cleanupsRunInLifoOrderExactlyOnce() {
        Harness screen = new Harness();
        List<String> events = screen.events;
        screen.register(() -> events.add("first"));
        screen.register(() -> events.add("second"));
        screen.register(() -> events.add("third"));

        screen.removed();
        screen.removed();

        assertEquals(List.of("third", "second", "first"), events,
            "cleanup 应按注册逆序执行，重复 removed 不得再次执行");
    }

    @Test
    void removalRunsBusinessHookBeforeCleanupAndStillReachesParentRemoval() {
        Harness screen = new Harness();
        screen.onRemovedAction = () -> screen.events.add("onRemoved");
        screen.register(() -> screen.events.add("cleanup"));

        screen.removed();

        assertEquals(List.of("onRemoved", "cleanup"), screen.events,
            "business hook 必须先于 cleanup，且 removed 应正常完成父类生命周期调用");
    }

    @Test
    void businessFailureDoesNotBypassCleanupOrParentRemoval() {
        RuntimeException primary = new RuntimeException("business failure");
        RuntimeException cleanupFailure = new RuntimeException("cleanup failure");
        Harness screen = new Harness();
        screen.onRemovedAction = () -> {
            screen.events.add("onRemoved");
            throw primary;
        };
        screen.register(() -> screen.events.add("cleanup-ok"));
        screen.register(() -> {
            screen.events.add("cleanup-fails");
            throw cleanupFailure;
        });

        RuntimeException thrown = assertThrows(RuntimeException.class, screen::removed);

        assertSame(primary, thrown, "首个 business 异常必须保持为 primary");
        assertSame(cleanupFailure, thrown.getSuppressed()[0],
            "cleanup 异常必须按执行顺序挂到 primary 的 suppressed 列表");
        assertEquals(List.of("onRemoved", "cleanup-fails", "cleanup-ok"), screen.events,
            "business 异常不得跳过任一 cleanup 或父类 removed 阶段");
    }

    @Test
    void multipleCleanupFailuresPreservePrimaryAndSuppressedOrder() {
        RuntimeException latest = new RuntimeException("latest cleanup");
        RuntimeException middle = new RuntimeException("middle cleanup");
        RuntimeException earliest = new RuntimeException("earliest cleanup");
        Harness screen = new Harness();
        screen.register(() -> {
            screen.events.add("earliest");
            throw earliest;
        });
        screen.register(() -> {
            screen.events.add("middle");
            throw middle;
        });
        screen.register(() -> {
            screen.events.add("latest");
            throw latest;
        });

        RuntimeException thrown = assertThrows(RuntimeException.class, screen::removed);

        assertSame(latest, thrown, "LIFO 中第一个失败的 cleanup 必须是 primary");
        assertEquals(List.of(middle, earliest), List.of(thrown.getSuppressed()),
            "后续 cleanup 异常必须按实际执行顺序 suppressed");
        assertEquals(List.of("latest", "middle", "earliest"), screen.events,
            "cleanup 异常不能截断剩余 cleanup");
    }

    @Test
    void repeatedRemovalIsANoopAfterTerminalFailureToo() {
        AtomicInteger removedCalls = new AtomicInteger();
        AtomicInteger cleanupCalls = new AtomicInteger();
        Harness screen = new Harness();
        screen.onRemovedAction = () -> {
            removedCalls.incrementAndGet();
            throw new IllegalStateException("terminal");
        };
        screen.register(cleanupCalls::incrementAndGet);

        assertThrows(IllegalStateException.class, screen::removed);
        assertDoesNotThrow(screen::removed, "第一次失败后重复 removed 仍必须是 no-op");
        assertEquals(1, removedCalls.get(), "terminal hook 必须 exactly-once");
        assertEquals(1, cleanupCalls.get(), "cleanup 必须 exactly-once");
    }

    @Test
    void queuedRefreshIsAcceptedBeforeRemovalAndDroppedWhenItArrivesAfterRemoval() {
        AtomicInteger refreshCalls = new AtomicInteger();
        Harness screen = new Harness();
        Runnable queuedBeforeRemoval = () -> screen.run(refreshCalls::incrementAndGet);
        queuedBeforeRemoval.run();
        assertEquals(1, refreshCalls.get(), "打开中的 queued refresh 应执行一次");

        Runnable queuedAfterRemoval = () -> screen.run(refreshCalls::incrementAndGet);
        screen.removed();
        queuedAfterRemoval.run();
        screen.run(refreshCalls::incrementAndGet);

        assertEquals(1, refreshCalls.get(),
            "removed 后才到达的 queued refresh 与直接调用都必须丢弃");
    }

    @Test
    void tickOnlyReachesHookWhileOpen() {
        Harness screen = new Harness();

        screen.tick();
        screen.tick();
        assertEquals(2, screen.tickCalls,
            "screen 打开时每个 tick 应到达 onScreenTick");

        screen.removed();
        screen.tick();
        assertEquals(2, screen.tickCalls,
            "removed 后 tick 不得再次触发 onScreenTick");
    }

    @Test
    void tickAndRemovalAreSerializedByTheLifecycleGate() throws Exception {
        Harness screen = new Harness();
        screen.onRemovedAction = () -> screen.events.add("onRemoved");
        CountDownLatch tickEntered = new CountDownLatch(1);
        CountDownLatch releaseTick = new CountDownLatch(1);
        CountDownLatch removalStarted = new CountDownLatch(1);
        screen.tickEntered = tickEntered;
        screen.releaseTick = releaseTick;

        Thread tickThread = new Thread(screen::tick, "bong-screen-test-tick");
        tickThread.start();
        assertTrue(tickEntered.await(1, TimeUnit.SECONDS),
            "测试 tick 必须先进入 hook，才能验证 removed 无法穿插");

        AtomicInteger removalCalls = new AtomicInteger();
        Thread removalThread = new Thread(() -> {
            removalStarted.countDown();
            screen.removed();
            removalCalls.incrementAndGet();
        }, "bong-screen-test-removal");
        removalThread.start();
        assertTrue(removalStarted.await(1, TimeUnit.SECONDS),
            "测试 removal 线程必须已发起生命周期调用");
        assertEquals(0, removalCalls.get(),
            "tick hook 持有生命周期锁时，removed 不得提前完成");

        releaseTick.countDown();
        tickThread.join(1_000);
        removalThread.join(1_000);
        assertFalse(tickThread.isAlive(), "tick 线程应在释放 hook 后完成");
        assertFalse(removalThread.isAlive(), "removed 线程应在 tick 完成后完成");
        assertEquals(1, removalCalls.get(), "removed 应在线性化后恰好执行一次");
        assertEquals(List.of("tick", "onRemoved"), screen.events,
            "生命周期锁应保证 removed 不会在 tick hook 中途穿插");
    }

    private static final class Harness extends BongScreenBase<ParentComponent> {
        private final List<String> events = new ArrayList<>();
        private Runnable onRemovedAction = () -> {
        };
        private int tickCalls;
        private CountDownLatch tickEntered;
        private CountDownLatch releaseTick;

        private Harness() {
            super();
        }

        private Harness(Text title) {
            super(title);
        }

        @Override
        protected @NotNull OwoUIAdapter<ParentComponent> createAdapter() {
            // Lifecycle tests never initialize the owo tree.  Returning the
            // injected adapter slot keeps this harness independent of a live
            // Minecraft window while preserving the production seam.
            return null;
        }

        @Override
        protected void build(ParentComponent rootComponent) {
        }

        private void register(Runnable cleanup) {
            registerCleanup(cleanup);
        }

        private void run(Runnable task) {
            runWhileOpen(task);
        }

        @Override
        protected void onScreenTick() {
            tickCalls++;
            if (tickEntered != null && releaseTick != null) {
                tickEntered.countDown();
                try {
                    releaseTick.await();
                } catch (InterruptedException interrupted) {
                    Thread.currentThread().interrupt();
                    throw new AssertionError(interrupted);
                }
            }
            events.add("tick");
        }

        @Override
        protected void onRemoved() {
            onRemovedAction.run();
        }
    }
}
