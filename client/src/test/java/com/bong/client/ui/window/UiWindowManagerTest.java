package com.bong.client.ui.window;

import org.junit.jupiter.api.Test;

import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.assertThrows;

class UiWindowManagerTest {
    @Test
    void presentationChangesKeepScopeAndPinUntilExplicitClose() {
        var manager = new UiWindowManager(300, 200);
        var key = manager.key("item-inspect", "presentation");
        var state = manager.openOrFocus(DEFINITION, key, new UiWindowManager.Rect(10, 10, 80, 60));
        var closed = new java.util.concurrent.atomic.AtomicInteger();
        state.scope().addCleanup(closed::incrementAndGet);
        manager.beginDrag(15, 15);
        manager.pin(key, true);
        manager.minimize(key);
        assertNull(manager.capturedKey());
        assertNull(manager.hitTest(20, 20), "最小化窗口不能继续拦截工作台输入");
        assertTrue(state.pinned());
        manager.cancelCapture();
        assertEquals(0, closed.get(), "最小化、固定和离开宿主不得结算业务");
        assertSame(state, manager.openOrFocus(DEFINITION, key, new UiWindowManager.Rect(0, 0, 60, 40)));
        assertFalse(state.minimized());
        assertTrue(state.pinned());
        manager.close(key);
        manager.close(key);
        assertEquals(1, closed.get(), "明确关闭只能执行一次 cleanup");
    }

    @Test
    void invalidSizeDoesNotMutateAndViewportChangePreservesRequestedSize() {
        var manager = new UiWindowManager(300, 200);
        var key = manager.key("item-inspect", "size");
        var state = manager.openOrFocus(DEFINITION, key, new UiWindowManager.Rect(5, 5, 80, 60));
        for (String invalid : new String[] {"", "-1", "0", "NaN", "12.5", "2147483648"}) {
            assertFalse(manager.resize(key, invalid, "90"));
            assertEquals(new UiWindowManager.Rect(5, 5, 80, 60), state.bounds());
        }
        assertTrue(manager.resize(key, "450", "280"));
        assertEquals(300, state.bounds().width());
        manager.resizeViewport(600, 400);
        assertEquals(450, state.bounds().width(), "临时小 viewport 不应覆盖玩家期望尺寸");
        assertEquals(280, state.bounds().height());
    }

    private static final UiWindowDefinition DEFINITION = new UiWindowDefinition(
        "item-inspect", "item-inspect", 40, 30, Set.of(UiWindowDefinition.Capability.WINDOW)
    );

    @Test
    void openingTheSameIdentityFocusesWithoutCreatingAnotherScope() {
        UiWindowManager manager = new UiWindowManager(200, 120);
        UiWindowManager.WindowKey key = key("one");

        UiWindowManager.WindowState first = manager.openOrFocus(DEFINITION, key, new UiWindowManager.Rect(5, 5, 60, 40));
        UiWindowManager.WindowState reopened = manager.openOrFocus(DEFINITION, key, new UiWindowManager.Rect(90, 60, 60, 40));

        assertSame(first, reopened);
        assertEquals(1, manager.snapshot().size());
        assertFalse(first.scope().isClosed());
    }

    @Test
    void hitTestAndDragUseTheTopmostWindowAndClampToViewport() {
        UiWindowManager manager = new UiWindowManager(100, 80);
        UiWindowManager.WindowKey bottomKey = key("bottom");
        UiWindowManager.WindowKey topKey = key("top");
        manager.openOrFocus(DEFINITION, bottomKey, new UiWindowManager.Rect(10, 10, 60, 40));
        UiWindowManager.WindowState top = manager.openOrFocus(DEFINITION, topKey, new UiWindowManager.Rect(20, 20, 60, 40));

        assertEquals(topKey, manager.hitTest(30, 30).key());
        assertTrue(manager.beginDrag(30, 30));
        assertTrue(manager.dragTo(99, 79));
        assertEquals(new UiWindowManager.Rect(40, 40, 60, 40), top.bounds());
        assertTrue(manager.endDrag());
        assertFalse(manager.endDrag());
    }

    @Test
    void closeReleasesScopeAndCapture() {
        UiWindowManager manager = new UiWindowManager(100, 80);
        UiWindowManager.WindowKey key = key("close");
        UiWindowManager.WindowState state = manager.openOrFocus(DEFINITION, key, new UiWindowManager.Rect(0, 0, 50, 30));

        assertTrue(manager.beginDrag(2, 2));
        assertTrue(manager.close(key));
        assertTrue(state.closed());
        assertTrue(state.scope().isClosed());
        assertNull(manager.capturedKey());
        assertFalse(manager.close(key));
        var reopened = manager.openOrFocus(DEFINITION, key, new UiWindowManager.Rect(0, 0, 50, 30));
        assertFalse(state.scope().runIfOpen(() -> manager.close(reopened.key())),
            "已关闭窗口的迟到回调不能关闭后来重开的同 identity 窗口");
        assertFalse(reopened.closed());
    }

    @Test
    void tinyViewportClipsEffectiveSizeAndRestoresRequestedSize() {
        var manager = new UiWindowManager(100, 80);
        var state = manager.openOrFocus(DEFINITION, key("resize"), new UiWindowManager.Rect(10, 10, 70, 50));
        manager.resizeViewport(20, 15);
        assertEquals(new UiWindowManager.Rect(0, 0, 20, 15), state.bounds());
        manager.resizeViewport(100, 80);
        assertEquals(new UiWindowManager.Rect(10, 10, 70, 50), state.bounds(),
            "临时缩小 viewport 不能永久丢失期望尺寸");
    }

    @Test
    void resetRevokesOldKeysAndCleansEveryWindowEvenIfOneCleanupFails() {
        var manager = new UiWindowManager(100, 80);
        var first = manager.openOrFocus(DEFINITION, key("first"), new UiWindowManager.Rect(0, 0, 50, 30));
        var second = manager.openOrFocus(DEFINITION, key("second"), new UiWindowManager.Rect(0, 0, 50, 30));
        var failure = new IllegalStateException("cleanup failed");
        second.scope().addCleanup(() -> { throw failure; });
        manager.beginDrag(2, 2);
        assertSame(failure, assertThrows(IllegalStateException.class, manager::reset));
        assertTrue(first.scope().isClosed());
        assertTrue(second.scope().isClosed());
        assertTrue(manager.snapshot().isEmpty());
        assertNull(manager.capturedKey());
        assertThrows(IllegalStateException.class,
            () -> manager.openOrFocus(DEFINITION, first.key(), first.bounds()));
        var next = manager.openOrFocus(DEFINITION, manager.key("item-inspect", "first"), first.bounds());
        assertFalse(next.closed());
    }

    @Test
    void cancelStopsCapturedDragOutsideTheWindow() {
        var manager = new UiWindowManager(100, 80);
        var state = manager.openOrFocus(DEFINITION, key("drag"), new UiWindowManager.Rect(10, 10, 50, 30));
        manager.beginDrag(12, 12);
        manager.cancelCapture();
        assertFalse(manager.dragTo(90, 70));
        assertEquals(new UiWindowManager.Rect(10, 10, 50, 30), state.bounds());
    }

    private static UiWindowManager.WindowKey key(String identity) {
        return new UiWindowManager.WindowKey("item-inspect", 0L, identity);
    }
}
