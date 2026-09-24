package com.bong.client.ui.contract.surface;

import org.junit.jupiter.api.Test;

import java.util.LinkedHashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UiSurfaceProjectionTest {
    @Test
    void projectionCopiesMapsAndActionSchemaValidatesAllBranches() {
        Map<String, String> viewData = new LinkedHashMap<>();
        viewData.put("title", "炼器");
        UiActionSpec action = new UiActionSpec(
            "craft.start",
            Map.of("recipe", UiActionSpec.ArgumentType.STRING, "count", UiActionSpec.ArgumentType.INTEGER),
            true,
            null
        );
        UiSurfaceProjection projection = new UiSurfaceProjection(
            "surface-1", "craft", "session-1", 3L, 100L, null,
            viewData, Map.of("row-1", "instance-1"), Map.of(action.actionId(), action)
        );
        viewData.put("mutated", "caller");
        assertEquals(Map.of("title", "炼器"), projection.viewData());
        assertThrows(UnsupportedOperationException.class,
            () -> projection.viewData().put("late", "mutation"));
        assertTrue(action.validate(Map.of("recipe", "pill", "count", 2)).valid());
        assertFalse(action.validate(Map.of("recipe", "pill")).valid());
        assertFalse(action.validate(Map.of("recipe", "pill", "count", "2")).valid());
        assertFalse(action.validate(Map.of("recipe", "pill", "count", 2, "extra", true)).valid());
    }

    @Test
    void closedExpiredAndUnavailableSurfacesAreExplicit() {
        UiActionSpec unavailable = new UiActionSpec(
            "craft.cancel", Map.of(), false, "没有可取消的炼器会话");
        assertEquals("没有可取消的炼器会话", unavailable.validate(Map.of()).reason());
        assertThrows(IllegalArgumentException.class,
            () -> new UiActionSpec("craft.cancel", Map.of(), false, null));
        assertThrows(IllegalArgumentException.class,
            () -> new UiActionSpec("craft.cancel", Map.of(), true, "reason"));

        UiSurfaceProjection surface = new UiSurfaceProjection(
            "surface-1", "craft", "session-1", 0L, 10L, "closed",
            Map.of(), Map.of(), Map.of(unavailable.actionId(), unavailable)
        );
        assertTrue(surface.isClosed());
        assertTrue(surface.isExpired(10L));
        assertEquals(unavailable, surface.action("craft.cancel"));
        assertThrows(IllegalArgumentException.class,
            () -> new UiSurfaceProjection("s", "t", "session", -1L, -1L, null,
                Map.of(), Map.of(), Map.of()));
    }
}
