package com.bong.client.network;

import com.bong.client.insight.InsightCategory;
import com.bong.client.insight.InsightChoice;
import com.bong.client.insight.InsightOfferStore;
import com.bong.client.insight.InsightOfferViewModel;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

public final class HeartDemonOfferHandler implements ServerDataHandler {
    private static final long FALLBACK_TIMEOUT_MILLIS = 30_000L;

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String triggerId = readString(payload, "trigger_id");
        if (triggerId == null || triggerId.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring heart_demon_offer payload: required field 'trigger_id' is missing or invalid"
            );
        }
        List<InsightChoice> choices = readChoices(payload);
        if (choices.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring heart_demon_offer payload: choices are missing or invalid"
            );
        }

        long nowMillis = System.currentTimeMillis();
        long expiresAtMillis = readLong(payload, "expires_at_ms", 0L);
        if (expiresAtMillis <= nowMillis) {
            expiresAtMillis = nowMillis + FALLBACK_TIMEOUT_MILLIS;
        }
        InsightOfferStore.replace(new InsightOfferViewModel(
            triggerId,
            fallback(readString(payload, "trigger_label"), "心魔劫临身"),
            fallback(readString(payload, "realm_label"), "渡虚劫 · 心魔"),
            clamp01(readDouble(payload, "composure", 0.5d)),
            Math.max(0, readInt(payload, "quota_remaining", 1)),
            Math.max(1, readInt(payload, "quota_total", 1)),
            expiresAtMillis,
            choices
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied heart_demon_offer '" + triggerId + "' to InsightOfferStore"
        );
    }

    private static List<InsightChoice> readChoices(JsonObject payload) {
        JsonElement choicesElement = payload.get("choices");
        if (choicesElement == null || !choicesElement.isJsonArray()) {
            return List.of();
        }
        JsonArray choicesArray = choicesElement.getAsJsonArray();
        List<InsightChoice> choices = new ArrayList<>();
        int limit = Math.min(4, choicesArray.size());
        for (int i = 0; i < limit; i++) {
            JsonElement element = choicesArray.get(i);
            if (!element.isJsonObject()) {
                continue;
            }
            JsonObject choice = element.getAsJsonObject();
            String choiceId = readString(choice, "choice_id");
            if (choiceId == null || choiceId.isBlank()) {
                choiceId = "heart_demon_choice_" + i;
            }
            choices.add(new InsightChoice(
                choiceId,
                parseCategory(readString(choice, "category")),
                fallback(readString(choice, "title"), "心魔"),
                fallback(readString(choice, "effect_summary"), "等待服务端结算"),
                fallback(readString(choice, "flavor"), "心魔无声逼近。"),
                fallback(readString(choice, "style_hint"), "抉择")
            ));
        }
        return choices;
    }

    private static InsightCategory parseCategory(String wire) {
        if (wire == null) {
            return InsightCategory.COMPOSURE;
        }
        return switch (wire) {
            case "Meridian" -> InsightCategory.MERIDIAN;
            case "Qi" -> InsightCategory.QI;
            case "Composure" -> InsightCategory.COMPOSURE;
            case "Coloring" -> InsightCategory.QI_COLOR;
            case "Breakthrough" -> InsightCategory.BREAKTHROUGH;
            case "Style" -> InsightCategory.SCHOOL;
            case "Perception" -> InsightCategory.PERCEPTION;
            default -> InsightCategory.COMPOSURE;
        };
    }

    private static String readString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }

    private static int readInt(JsonObject object, String fieldName, int fallback) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return fallback;
        }
        return primitive.getAsInt();
    }

    private static long readLong(JsonObject object, String fieldName, long fallback) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return fallback;
        }
        return primitive.getAsLong();
    }

    private static double readDouble(JsonObject object, String fieldName, double fallback) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return fallback;
        }
        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : fallback;
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }

    private static String fallback(String value, String fallback) {
        return value == null || value.isBlank() ? fallback : value;
    }

    private static double clamp01(double value) {
        return Math.max(0.0d, Math.min(1.0d, value));
    }
}
