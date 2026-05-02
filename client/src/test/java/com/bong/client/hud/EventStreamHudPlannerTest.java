package com.bong.client.hud;

import com.bong.client.combat.HudConfig;
import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStream;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.assertEquals;

class EventStreamHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @AfterEach
    void tearDown() {
        HudConfig.resetToDefaults();
    }

    @Test
    void visibleConfigRendersBufferedEvents() {
        UnifiedEventStream stream = streamWithOneEvent();

        List<HudRenderCommand> commands = EventStreamHudPlanner.buildCommands(
            stream, 1_000L, FIXED_WIDTH, 1920, 1080);

        assertFalse(commands.isEmpty(), "event stream should render when hud.event_stream.visible is enabled");
    }

    @Test
    void hiddenConfigKeepsEventsBufferedButEmitsNoCommands() {
        UnifiedEventStream stream = streamWithOneEvent();
        HudConfig.setEventStreamVisible(false);

        List<HudRenderCommand> commands = EventStreamHudPlanner.buildCommands(
            stream, 8_000L, FIXED_WIDTH, 1920, 1080);

        assertTrue(commands.isEmpty(), "hidden event stream must not draw panel or text commands");
        assertEquals(1, stream.size(), "hidden event stream keeps buffered events for re-open");
    }

    private static UnifiedEventStream streamWithOneEvent() {
        UnifiedEventStream stream = new UnifiedEventStream();
        stream.publish(UnifiedEvent.Channel.COMBAT, UnifiedEvent.Priority.P2_NORMAL,
            "wolf", "命中 野狼", 0, 1_000L);
        return stream;
    }
}
