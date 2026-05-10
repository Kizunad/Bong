package com.bong.client.combat.handler;

import com.bong.client.combat.store.DerivedAttrsStore;
import com.bong.client.combat.store.FalseSkinHudStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

/**
 * Handles {@code false_skin_state} payloads from plan-tuike-v1.
 */
public final class FalseSkinStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        int layers = (int) Math.max(0L, Math.round(readDouble(payload, "layers_remaining", 0d)));
        String targetId = readString(payload, "target_id", "");
        String kind = readString(payload, "kind", "");
        float contamCapacity = readNonNegativeFloat(payload, "contam_capacity_per_layer", 0d);
        float absorbedContam = readNonNegativeFloat(payload, "absorbed_contam", 0d);
        long equippedAtTick = Math.max(0L, Math.round(readDouble(payload, "equipped_at_tick", 0d)));

        FalseSkinHudStateStore.State nextFalseSkinState = new FalseSkinHudStateStore.State(
            targetId,
            kind,
            layers,
            contamCapacity,
            absorbedContam,
            equippedAtTick,
            readLayers(payload)
        );
        FalseSkinHudStateStore.replace(nextFalseSkinState);
        int normalizedLayers = nextFalseSkinState.layersRemaining();

        DerivedAttrsStore.State current = DerivedAttrsStore.snapshot();
        DerivedAttrsStore.replace(new DerivedAttrsStore.State(
            current.flying(),
            current.flyingQiRemaining(),
            current.flyingForceDescentAtMs(),
            current.phasing(),
            current.phasingUntilMs(),
            current.tribulationLocked(),
            current.tribulationStage(),
            current.throughputPeakNorm(),
            normalizedLayers,
            current.vortexActive()
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied false_skin_state (layers_remaining=" + normalizedLayers + ")"
        );
    }

    private static List<FalseSkinHudStateStore.Layer> readLayers(JsonObject obj) {
        JsonElement el = obj.get("layers");
        if (el == null || el.isJsonNull() || !el.isJsonArray()) return List.of();
        JsonArray array = el.getAsJsonArray();
        List<FalseSkinHudStateStore.Layer> layers = new ArrayList<>();
        for (JsonElement item : array) {
            if (!item.isJsonObject()) continue;
            JsonObject layer = item.getAsJsonObject();
            layers.add(new FalseSkinHudStateStore.Layer(
                readString(layer, "tier", "fan"),
                readNonNegativeFloat(layer, "spirit_quality", 1d),
                readNonNegativeFloat(layer, "damage_capacity", 0d),
                readNonNegativeFloat(layer, "contam_load", 0d),
                readNonNegativeFloat(layer, "permanent_taint_load", 0d)
            ));
            if (layers.size() >= 3) break;
        }
        return List.copyOf(layers);
    }

    private static String readString(JsonObject obj, String field, String fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isString()) return fallback;
        String value = p.getAsString();
        return value == null ? fallback : value;
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double v = p.getAsDouble();
        return Double.isFinite(v) ? v : fallback;
    }

    private static float readNonNegativeFloat(JsonObject obj, String field, double fallback) {
        return (float) Math.max(0d, readDouble(obj, field, fallback));
    }
}
