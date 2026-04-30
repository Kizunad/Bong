package com.bong.client.network;

import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.util.ArrayList;
import java.util.List;

public final class BotanyHarvestProgressHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String sessionId = readOptionalString(payload, "session_id");
        if (sessionId == null || sessionId.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring botany_harvest_progress payload: required field 'session_id' is missing or invalid"
            );
        }
        Double progress = readOptionalDouble(payload, "progress");

        HarvestSessionViewModel model = HarvestSessionViewModel.create(
            sessionId,
            readOptionalString(payload, "target_id"),
            readOptionalString(payload, "target_name"),
            readOptionalString(payload, "plant_kind"),
            BotanyHarvestMode.fromWireName(readOptionalString(payload, "mode")),
            progress == null ? 0.0 : progress,
            readOptionalBoolean(payload, "auto_selectable") != Boolean.FALSE,
            readOptionalBoolean(payload, "request_pending") == Boolean.TRUE,
            readOptionalBoolean(payload, "interrupted") == Boolean.TRUE,
            readOptionalBoolean(payload, "completed") == Boolean.TRUE,
            readOptionalString(payload, "detail"),
            readHazardHints(payload),
            readOptionalDoubleTriple(payload, "target_pos"),
            System.currentTimeMillis()
        );

        HarvestSessionStore.replace(model);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied botany_harvest_progress session '" + model.sessionId() + "' to HarvestSessionStore"
        );
    }

    private static String readOptionalString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }

    private static Double readOptionalDouble(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        return primitive.getAsDouble();
    }

    private static Boolean readOptionalBoolean(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isBoolean()) {
            return null;
        }
        return primitive.getAsBoolean();
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }

    private static double[] readOptionalDoubleTriple(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || !element.isJsonArray()) {
            return null;
        }
        JsonArray array = element.getAsJsonArray();
        if (array.size() != 3) {
            return null;
        }
        double[] out = new double[3];
        for (int i = 0; i < 3; i++) {
            JsonElement el = array.get(i);
            if (!el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) {
                return null;
            }
            out[i] = el.getAsDouble();
        }
        return out;
    }

    private static List<String> readHazardHints(JsonObject object) {
        List<String> hints = new ArrayList<>();
        String single = readOptionalString(object, "hazard_hint");
        if (single != null && !single.isBlank()) {
            hints.add(single);
        }
        JsonElement element = object.get("hazard_hints");
        if (element != null && element.isJsonArray()) {
            for (JsonElement item : element.getAsJsonArray()) {
                if (item != null && item.isJsonPrimitive() && item.getAsJsonPrimitive().isString()) {
                    String value = item.getAsString();
                    if (!value.isBlank()) {
                        hints.add(value);
                    }
                }
            }
        }
        return List.copyOf(hints);
    }
}
