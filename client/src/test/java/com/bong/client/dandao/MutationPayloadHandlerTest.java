package com.bong.client.dandao;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-dandao-path-v1 P3 -- Tests for MutationPayloadHandler deserialization.
 * Tests the round-trip: JSON -> handle() -> MutationVisualState.
 */
class MutationPayloadHandlerTest {
    @AfterEach
    void cleanup() {
        MutationVisualState.reset();
    }

    @Test
    void handleFullPayload() {
        JsonObject json = new JsonObject();
        json.addProperty("stage", 2);
        json.addProperty("cumulative_toxin", 175.5);
        json.addProperty("meridian_penalty", 0.08);

        JsonArray slots = new JsonArray();
        JsonObject slot1 = new JsonObject();
        slot1.addProperty("kind", "GoldenIris");
        slot1.addProperty("body_slot", "Head");
        slot1.addProperty("level", 1);
        slots.add(slot1);

        JsonObject slot2 = new JsonObject();
        slot2.addProperty("kind", "ForearmScales");
        slot2.addProperty("body_slot", "Forearm");
        slot2.addProperty("level", 2);
        slots.add(slot2);

        json.add("slots", slots);

        MutationPayloadHandler.handle(json);

        assertEquals(2, MutationVisualState.stage());
        assertEquals(175.5, MutationVisualState.cumulativeToxin(), 0.001);
        assertEquals(0.08, MutationVisualState.meridianPenalty(), 0.001);
        assertEquals(2, MutationVisualState.activeSlots().size());
        assertEquals("GoldenIris", MutationVisualState.activeSlots().get(0).kind());
        assertEquals("ForearmScales", MutationVisualState.activeSlots().get(1).kind());
    }

    @Test
    void handleEmptyPayloadDefaultsToZero() {
        JsonObject json = new JsonObject();
        MutationPayloadHandler.handle(json);

        assertEquals(0, MutationVisualState.stage());
        assertEquals(0.0, MutationVisualState.cumulativeToxin(), 0.001);
        assertTrue(MutationVisualState.activeSlots().isEmpty());
    }

    @Test
    void handlePayloadWithNoSlots() {
        JsonObject json = new JsonObject();
        json.addProperty("stage", 1);
        json.addProperty("cumulative_toxin", 45.0);
        json.addProperty("meridian_penalty", 0.03);

        MutationPayloadHandler.handle(json);

        assertEquals(1, MutationVisualState.stage());
        assertEquals(45.0, MutationVisualState.cumulativeToxin(), 0.001);
        assertTrue(MutationVisualState.activeSlots().isEmpty());
    }

    @Test
    void handlePayloadWithMissingSlotFields() {
        JsonObject json = new JsonObject();
        json.addProperty("stage", 3);

        JsonArray slots = new JsonArray();
        JsonObject slot = new JsonObject();
        // Missing kind, body_slot, level -- should default
        slots.add(slot);
        json.add("slots", slots);

        MutationPayloadHandler.handle(json);

        assertEquals(3, MutationVisualState.stage());
        assertEquals(1, MutationVisualState.activeSlots().size());
        assertEquals("Unknown", MutationVisualState.activeSlots().get(0).kind());
        assertEquals("Unknown", MutationVisualState.activeSlots().get(0).bodySlot());
        assertEquals(1, MutationVisualState.activeSlots().get(0).level());
    }

    @Test
    void handlePayloadWithStage4() {
        JsonObject json = new JsonObject();
        json.addProperty("stage", 4);
        json.addProperty("cumulative_toxin", 650.0);
        json.addProperty("meridian_penalty", 0.20);

        JsonArray slots = new JsonArray();
        JsonObject slot = new JsonObject();
        slot.addProperty("kind", "ExtraArms");
        slot.addProperty("body_slot", "Torso");
        slot.addProperty("level", 3);
        slots.add(slot);
        json.add("slots", slots);

        MutationPayloadHandler.handle(json);

        assertEquals(4, MutationVisualState.stage());
        assertEquals("ExtraArms", MutationVisualState.activeSlots().get(0).kind());
    }
}
