package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ClientThreadMarshalTest {
    @Test
    void clientThreadRunsInlineExactlyOnce() {
        AtomicInteger runs = new AtomicInteger();

        assertTrue(ClientThreadMarshal.run(() -> true, runs::incrementAndGet, ignored -> {
            throw new AssertionError("client-thread work must not be queued");
        }));
        assertEquals(1, runs.get(), "客户端线程任务必须只内联执行一次");
    }

    @Test
    void offThreadQueuesExactlyOnceWithoutInlineExecution() {
        AtomicInteger runs = new AtomicInteger();
        List<Runnable> queued = new ArrayList<>();

        assertTrue(ClientThreadMarshal.run(() -> false, runs::incrementAndGet, queued::add));
        assertEquals(0, runs.get(), "非客户端线程不能提前执行任务");
        assertEquals(1, queued.size(), "非客户端线程必须只入队一次");
        queued.get(0).run();
        assertEquals(1, runs.get(), "执行队列任务后应只产生一次副作用");
    }

    @Test
    void unknownThreadStateFailsClosedWithoutExecutingOrEnqueuing() {
        AtomicInteger runs = new AtomicInteger();
        List<Runnable> queued = new ArrayList<>();

        assertFalse(ClientThreadMarshal.run(() -> null, runs::incrementAndGet, queued::add));
        assertEquals(0, runs.get(), "未知线程状态不能执行任务");
        assertTrue(queued.isEmpty(), "未知线程状态不能留下不可审计的队列任务");
    }

    @Test
    void taskFailureIsNotSwallowed() {
        IllegalStateException failure = new IllegalStateException("task failed");

        assertThrows(IllegalStateException.class,
            () -> ClientThreadMarshal.run(() -> true, () -> { throw failure; }, ignored -> { }),
            "任务异常必须原样传播给调用方");
    }
}
