package com.bong.client.network;

import com.bong.client.state.SeasonState;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Optional;

final class SeasonStatePayload {
    private SeasonStatePayload() {
    }

    static Optional<SeasonState> readOptional(JsonObject payload, String fieldName) {
        JsonObject state = readObject(payload, fieldName);
        if (state == null) {
            return Optional.empty();
        }

        Optional<SeasonState.Phase> phase = SeasonState.Phase.fromWire(readString(state, "season"));
        Long tickIntoPhase = readLong(state, "tick_into_phase");
        Long phaseTotalTicks = readLong(state, "phase_total_ticks");
        Long yearIndex = readLong(state, "year_index");
        if (phase.isEmpty() || tickIntoPhase == null || phaseTotalTicks == null || yearIndex == null) {
            return Optional.empty();
        }

        return Optional.of(new SeasonState(phase.get(), tickIntoPhase, phaseTotalTicks, yearIndex));
    }

    private static JsonObject readObject(JsonObject payload, String fieldName) {
        JsonElement element = payload == null ? null : payload.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonObject()) {
            return null;
        }
        return element.getAsJsonObject();
    }

    private static String readString(JsonObject payload, String fieldName) {
        JsonElement element = payload == null ? null : payload.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isString() ? primitive.getAsString() : null;
    }

    private static Long readLong(JsonObject payload, String fieldName) {
        JsonElement element = payload == null ? null : payload.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        long value = primitive.getAsLong();
        return value >= 0L ? value : null;
    }
}
