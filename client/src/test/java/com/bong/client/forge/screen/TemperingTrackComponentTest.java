package com.bong.client.forge.screen;

import com.bong.client.forge.state.ForgeSessionStore;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class TemperingTrackComponentTest {
    @Test
    void renders_pattern_from_store() {
        TemperingTrackComponent.RenderState state = TemperingTrackComponent.renderStateFrom(snapshot(
            "{\"step\":\"tempering\",\"pattern\":[\"L\",\"H\",\"F\"],\"beat_cursor\":1,\"hits\":1,\"misses\":0,\"deviation\":0}"
        ));

        assertEquals(List.of("H", "F"), state.patternRemaining());
        assertEquals(1, state.beatCursor());
    }

    @Test
    void combo_displays_correctly() {
        TemperingTrackComponent.RenderState state = TemperingTrackComponent.renderStateFrom(snapshot(
            "{\"step\":\"tempering\",\"pattern_remaining\":[\"L\"],\"hits\":4,\"misses\":1,\"deviation\":1}"
        ));

        assertEquals(4, state.combo());
        assertEquals(1, state.misses());
    }

    @Test
    void deviation_bar_at_max_shows_red() {
        assertEquals(0xFFFF5555, TemperingTrackComponent.deviationColor(8));
        assertEquals(0xFFFF5555, TemperingTrackComponent.deviationColor(12));
    }

    @Test
    void empty_pattern_shows_done_state() {
        TemperingTrackComponent.RenderState state = TemperingTrackComponent.renderStateFrom(snapshot(
            "{\"step\":\"tempering\",\"pattern\":[\"L\"],\"beat_cursor\":1,\"hits\":1,\"misses\":0,\"deviation\":0}"
        ));

        assertTrue(state.patternRemaining().isEmpty());
    }

    private static ForgeSessionStore.Snapshot snapshot(String stepStateJson) {
        return new ForgeSessionStore.Snapshot(
            7,
            "qing_feng_v0",
            "青锋剑",
            true,
            "tempering",
            1,
            2,
            stepStateJson
        );
    }
}
