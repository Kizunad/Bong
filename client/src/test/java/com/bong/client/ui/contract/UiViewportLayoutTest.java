package com.bong.client.ui.contract;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UiViewportLayoutTest {
    @Test
    void viewportModesAndMinimumBoundaryAreExplicit() {
        assertEquals(UiViewport.Mode.COMPACT, new UiViewport(320, 240, 1, 1.0d).mode());
        assertEquals(UiViewport.Mode.REGULAR, new UiViewport(854, 480, 2, 1.25d).mode());
        assertEquals(UiViewport.Mode.WIDE, new UiViewport(1280, 720, 3, 2.0d).mode());
        assertFalse(new UiViewport(320, 240, 1, 1.0d).belowMinimum());
        assertTrue(new UiViewport(319, 240, 1, 1.0d).belowMinimum());
    }

    @Test
    void coordinateConversionRoundTripsAcrossWindowScale() {
        UiViewport viewport = new UiViewport(1280, 720, 2, 1.25d);
        UiViewport.Point logical = new UiViewport.Point(321.5d, 177.25d);
        assertEquals(logical, viewport.physicalToLogical(viewport.logicalToPhysical(logical)));
        UiViewport.Rect safe = viewport.safeRect(20.0d, 30.0d);
        assertEquals(20.0d, safe.x());
        assertEquals(30.0d, safe.y());
        assertTrue(safe.contains(new UiViewport.Point(20.0d, 30.0d)));
    }

    @Test
    void layoutClampsBoundsHitRegionAndReportsTextOverflow() {
        UiViewport viewport = new UiViewport(320, 240, 1, 1.0d);
        UiLayoutPolicy.LayoutSnapshot layout = UiLayoutPolicy.centered(
            viewport,
            new UiLayoutPolicy.Request(600.0d, 500.0d, 200.0d, 20.0d, 8.0d, 8.0d)
        );
        assertEquals(UiViewport.Mode.COMPACT, layout.mode());
        assertFalse(layout.belowMinimum(), "最低支持尺寸本身仍属于完整支持边界");
        assertFalse(layout.textOverflow(), "safe rect 足以容纳 text 时不应误报溢出");
        assertTrue(layout.hitRegionClipped(), "超出 safe rect 的 hit region 必须被裁剪");
        assertEquals(layout.hitRegion(), layout.safeRect().intersection(layout.hitRegion()));

        UiLayoutPolicy.LayoutSnapshot textOverflow = UiLayoutPolicy.centered(
            viewport,
            new UiLayoutPolicy.Request(100.0d, 100.0d, 500.0d, 0.0d, 0.0d, 0.0d)
        );
        assertTrue(textOverflow.textOverflow());
    }

    @Test
    void invalidViewportAndLayoutArgumentsFailWithBoundaries() {
        assertThrows(IllegalArgumentException.class, () -> new UiViewport(0, 240, 1, 1.0d));
        assertThrows(IllegalArgumentException.class, () -> new UiViewport(320, 240, 1, 0.0d));
        assertThrows(IllegalArgumentException.class,
            () -> new UiViewport(320, 240, 1, 1.0d).safeRect(-1.0d, 0.0d));
        assertThrows(IllegalArgumentException.class,
            () -> new UiLayoutPolicy.Request(1.0d, 1.0d, 1.0d, -1.0d, 0.0d, 0.0d));
    }
}
