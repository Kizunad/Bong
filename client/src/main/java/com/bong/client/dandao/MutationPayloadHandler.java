package com.bong.client.dandao;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-dandao-path-v1 P3 — Parses "bong:mutation_visual" CustomPayload JSON
 * and updates MutationVisualState.
 *
 * Expected JSON shape:
 * {
 *   "stage": 2,
 *   "cumulative_toxin": 123.4,
 *   "meridian_penalty": 0.08,
 *   "slots": [
 *     { "kind": "GoldenIris", "body_slot": "Head", "level": 1 },
 *     ...
 *   ]
 * }
 */
public final class MutationPayloadHandler {
    private MutationPayloadHandler() {}

    public static void handle(JsonObject json) {
        int stage = json.has("stage") ? json.get("stage").getAsInt() : 0;
        double toxin = json.has("cumulative_toxin") ? json.get("cumulative_toxin").getAsDouble() : 0.0;
        double penalty = json.has("meridian_penalty") ? json.get("meridian_penalty").getAsDouble() : 0.0;

        List<MutationVisualState.MutationSlotEntry> slots = new ArrayList<>();
        if (json.has("slots") && json.get("slots").isJsonArray()) {
            JsonArray arr = json.getAsJsonArray("slots");
            for (var el : arr) {
                JsonObject slot = el.getAsJsonObject();
                String kind = slot.has("kind") ? slot.get("kind").getAsString() : "Unknown";
                String bodySlot = slot.has("body_slot") ? slot.get("body_slot").getAsString() : "Unknown";
                int level = slot.has("level") ? slot.get("level").getAsInt() : 1;
                slots.add(new MutationVisualState.MutationSlotEntry(kind, bodySlot, level));
            }
        }

        MutationVisualState.update(stage, toxin, penalty, slots);
    }
}
