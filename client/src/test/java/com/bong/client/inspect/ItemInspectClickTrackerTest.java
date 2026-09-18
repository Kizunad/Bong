package com.bong.client.inspect;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class ItemInspectClickTrackerTest {
    @Test
    void doubleClickOpensWithoutStartingADragAndDoesNotRepeatOnThirdClick() {
        var clicks = new ItemInspectClickTracker();
        assertFalse(clicks.press(42, 10, 10, 1000));
        assertNull(clicks.drag(11, 10), "点击抖动不能导致解绑或库存移动");
        assertTrue(clicks.release(11, 10, 1020));
        assertTrue(clicks.press(42, 11, 10, 1200));
        assertNull(clicks.drag(30, 10), "打开详情的第二击不再拖动物品");
        clicks.release(11, 10, 1220);
        assertFalse(clicks.press(42, 11, 10, 1300));
    }

    @Test
    void draggingReturnsOriginalPickupPositionAndCancelsDoubleClick() {
        var clicks = new ItemInspectClickTracker();
        clicks.press(42, 10, 10, 1000);
        assertEquals(new ItemInspectClickTracker.Press(42, 10, 10), clicks.drag(16, 10));
        assertNull(clicks.drag(20, 10), "一次拖动只能拾取一次");
        assertFalse(clicks.release(20, 10, 1020));
        assertFalse(clicks.press(42, 10, 10, 1100), "拖放后点击不能误触详情");
    }

    @Test
    void changedItemExpiredClickAndCancelledInputCannotOpenDetails() {
        var clicks = new ItemInspectClickTracker();
        clicks.press(42, 10, 10, 1000);
        clicks.release(10, 10, 1020);
        assertFalse(clicks.press(43, 10, 10, 1100), "同一位置换物品不能继承双击");
        clicks.release(10, 10, 1120);
        assertFalse(clicks.press(43, 10, 10, 1700));
        clicks.release(10, 10, 1720);
        clicks.cancel();
        assertFalse(clicks.press(43, 10, 10, 1800), "右键或关窗口后不能继承上一击");
    }
}
