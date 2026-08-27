package com.bong.client.ui.adapter.owo;

import com.bong.client.ui.contract.UiScreenScope;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Sizing;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class OwoXmlHostLifecycleTest {
    @Test
    void repeatedInitAndTemplateReloadOpenHostOnlyOnce() {
        OwoXmlHostLifecycle lifecycle = new OwoXmlHostLifecycle();
        AtomicInteger opens = new AtomicInteger();

        lifecycle.openOnce(scope -> opens.incrementAndGet());
        assertFalse(OwoXmlScreenHost.shouldReloadTemplate(true, "craft", "craft"));
        assertTrue(OwoXmlScreenHost.shouldReloadTemplate(true, "craft-compact", "craft"));
        lifecycle.openOnce(scope -> opens.incrementAndGet());

        assertEquals(1, opens.get(),
            "同模板 resize 和跨断点 XML 重建都不能重复执行 host open 或重复订阅");
    }

    @Test
    void closeIsIdempotentAndLateCallbacksAreNoOp() {
        OwoXmlHostLifecycle lifecycle = new OwoXmlHostLifecycle();
        AtomicInteger cleanups = new AtomicInteger();
        AtomicInteger hostCloses = new AtomicInteger();
        UiScreenScope scope = lifecycle.scope();
        lifecycle.openOnce(opened -> opened.addCleanup(cleanups::incrementAndGet));

        lifecycle.close(hostCloses::incrementAndGet);
        lifecycle.close(hostCloses::incrementAndGet);

        assertEquals(1, cleanups.get(), "scope cleanup 必须 exactly-once");
        assertEquals(1, hostCloses.get(), "host close 必须 exactly-once");
        assertTrue(lifecycle.isClosed());
        assertFalse(scope.runIfOpen(() -> {
            throw new AssertionError("关闭后的 late callback 不得执行");
        }));
        lifecycle.openOnce(opened -> {
            throw new AssertionError("关闭后的 host 不得重新打开");
        });
    }

    @Test
    void cleanupFailureDoesNotSkipHostCloseAndFailuresAreAggregated() {
        OwoXmlHostLifecycle lifecycle = new OwoXmlHostLifecycle();
        List<String> calls = new ArrayList<>();
        IllegalStateException cleanupFailure = new IllegalStateException("cleanup");
        IllegalArgumentException hostFailure = new IllegalArgumentException("host");
        lifecycle.openOnce(scope -> scope.addCleanup(() -> {
            calls.add("cleanup");
            throw cleanupFailure;
        }));

        Throwable thrown = assertThrows(Throwable.class, () -> lifecycle.close(() -> {
            calls.add("host");
            throw hostFailure;
        }));

        assertSame(cleanupFailure, thrown);
        assertEquals(List.of("cleanup", "host"), calls,
            "cleanup 失败后仍必须执行 host teardown");
        assertEquals(List.of(hostFailure), List.of(thrown.getSuppressed()));
        lifecycle.close(() -> {
            throw new AssertionError("重复关闭不得重跑失败阶段");
        });
    }

    @Test
    void requiredXmlIdIsResolvedThroughFailFastLookup() {
        FlowLayout root = Containers.verticalFlow(Sizing.fixed(20), Sizing.fixed(20));
        FlowLayout child = Containers.verticalFlow(Sizing.fixed(10), Sizing.fixed(10));
        child.id("present");
        root.child(child);

        assertSame(child, OwoXmlScreenHost.requireComponent(root, FlowLayout.class, "present"));
        IllegalStateException missing = assertThrows(IllegalStateException.class,
            () -> OwoXmlScreenHost.requireComponent(root, FlowLayout.class, "missing"));
        assertTrue(missing.getMessage().contains("missing XML component id: missing"));
    }

    @Test
    void reloadDecisionRequiresAnExistingAdapterAndAChangedTemplate() {
        assertFalse(OwoXmlScreenHost.shouldReloadTemplate(false, null, "craft"));
        assertFalse(OwoXmlScreenHost.shouldReloadTemplate(true, "craft", "craft"));
        assertTrue(OwoXmlScreenHost.shouldReloadTemplate(true, "craft", "craft-compact"));
    }
}
