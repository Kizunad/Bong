package com.bong.client.network;

import com.bong.client.hud.PillBuffHudPlanner;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.regex.Pattern;

/** 处理战斗丹 buff HUD 状态推送。 */
public final class PillBuffStatusHandler implements ServerDataHandler {
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String buffId = readString(p, "buff_id");
        Integer remainingTicks = readInteger(p, "remaining_ticks");
        Double effectMultiplier = readDouble(p, "effect_multiplier");
        if (buffId == null || buffId.isBlank() || remainingTicks == null || effectMultiplier == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring pill_buff_status: invalid or missing buff_id/remaining_ticks/effect_multiplier");
        }
        if (remainingTicks < 0 || effectMultiplier <= 0.0) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring pill_buff_status: invalid remaining_ticks/effect_multiplier");
        }
        PillBuffHudPlanner.updateBuff(buffId, remainingTicks, effectMultiplier);
        return ServerDataDispatch.handled(envelope.type(),
            "Applied pill_buff_status " + buffId + " ticks=" + remainingTicks);
    }

    private static String readString(JsonObject obj, String fieldName) {
        JsonElement element = obj.get(fieldName);
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isString() ? primitive.getAsString() : null;
    }

    private static Integer readInteger(JsonObject obj, String fieldName) {
        JsonElement element = obj.get(fieldName);
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        String rawValue = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(rawValue).matches()) {
            return null;
        }
        try {
            return Integer.parseInt(rawValue);
        } catch (NumberFormatException ignored) {
            return null;
        }
    }

    private static Double readDouble(JsonObject obj, String fieldName) {
        JsonElement element = obj.get(fieldName);
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        try {
            double value = primitive.getAsDouble();
            return Double.isFinite(value) ? value : null;
        } catch (NumberFormatException ignored) {
            return null;
        }
    }
}
