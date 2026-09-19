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
