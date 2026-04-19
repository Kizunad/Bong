package com.bong.client.botany;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyDragStateTest {
    @AfterEach
    void tearDown() {
        BotanyDragState.resetForTests();
    }

    @Test
    void pressOutsidePanelDoesNotConsume() {
        BotanyDragState.recordRenderedBounds(100, 100, 280, 200);
        boolean consumed = BotanyDragState.onLeftButton(1, 50.0, 50.0);
        assertFalse(consumed);
        assertFalse(BotanyDragState.isDragging());
    }

    @Test
    void pressInsidePanelStartsDragAndConsumes() {
        BotanyDragState.recordRenderedBounds(100, 100, 280, 200);
        boolean consumed = BotanyDragState.onLeftButton(1, 150.0, 150.0);
        assertTrue(consumed);
        assertTrue(BotanyDragState.isDragging());
    }

    @Test
    void draggingMouseAccumulatesDelta() {
        BotanyDragState.recordRenderedBounds(100, 100, 280, 200);
        BotanyDragState.onLeftButton(1, 150.0, 150.0);
        BotanyDragState.tickDrag(170.0, 180.0);
        assertEquals(20, BotanyDragState.deltaX());
        assertEquals(30, BotanyDragState.deltaY());
    }

    @Test
    void releaseEndsDragAndConsumes() {
        BotanyDragState.recordRenderedBounds(100, 100, 280, 200);
        BotanyDragState.onLeftButton(1, 150.0, 150.0);
        boolean consumed = BotanyDragState.onLeftButton(0, 170.0, 170.0);
        assertTrue(consumed);
        assertFalse(BotanyDragState.isDragging());
    }

    @Test
    void sessionChangeResetsDelta() {
        BotanyDragState.recordRenderedBounds(100, 100, 280, 200);
        BotanyDragState.onLeftButton(1, 150.0, 150.0);
        BotanyDragState.tickDrag(200.0, 200.0);
        BotanyDragState.onLeftButton(0, 200.0, 200.0);
        assertEquals(50, BotanyDragState.deltaX());

        BotanyDragState.maybeResetForSession("next-session");
        assertEquals(0, BotanyDragState.deltaX());
        assertEquals(0, BotanyDragState.deltaY());
    }

    @Test
    void releaseWithoutDraggingDoesNotConsume() {
        BotanyDragState.recordRenderedBounds(100, 100, 280, 200);
        boolean consumed = BotanyDragState.onLeftButton(0, 150.0, 150.0);
        assertFalse(consumed);
    }
}
