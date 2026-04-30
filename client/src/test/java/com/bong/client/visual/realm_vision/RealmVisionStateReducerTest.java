package com.bong.client.visual.realm_vision;

import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

class RealmVisionStateReducerTest {
    @Test
    void applyPayloadStoresCurrentAndTransition() {
        RealmVisionState state = RealmVisionStateReducer.apply(RealmVisionState.empty(), payload(30.0, 60.0, 4), 10L);
        assertFalse(state.isEmpty());
        assertEquals(30.0, state.current().fogStart());
        assertEquals(60.0, state.current().fogEnd());
        assertEquals(100, state.transitionTicks());
        assertEquals(4, state.serverViewDistanceChunks());
        assertEquals(10L, state.startedAtTick());
    }

    @Test
    void applyBreakthroughDiffPreservesPrevious() {
        RealmVisionState first = RealmVisionStateReducer.apply(RealmVisionState.empty(), payload(30.0, 60.0, 4), 10L);
        RealmVisionState second = RealmVisionStateReducer.apply(first, payload(240.0, 316.0, 20), 20L);
        assertEquals(30.0, second.previous().fogStart());
        assertEquals(240.0, second.current().fogStart());
    }

    static JsonObject payload(double fogStart, double fogEnd, int chunks) {
        JsonObject payload = new JsonObject();
        payload.addProperty("fog_start", fogStart);
        payload.addProperty("fog_end", fogEnd);
        payload.addProperty("fog_color_rgb", 0xB8B0A8);
        payload.addProperty("fog_shape", "Cylinder");
        payload.addProperty("vignette_alpha", 0.55);
        payload.addProperty("tint_color_argb", 0x0FF0EDE8);
        payload.addProperty("particle_density", 0.0);
        payload.addProperty("transition_ticks", 100);
        payload.addProperty("server_view_distance_chunks", chunks);
        payload.addProperty("post_fx_sharpen", 0.0);
        return payload;
    }
}
