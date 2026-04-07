package com.bong.client.network;

import com.bong.client.state.ZoneState;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Objects;
import java.util.function.LongSupplier;

public final class ZoneInfoHandler implements ServerDataHandler {
    private final LongSupplier nowMillisSupplier;

    public ZoneInfoHandler() {
        this(System::currentTimeMillis);
    }

    ZoneInfoHandler(LongSupplier nowMillisSupplier) {
        this.nowMillisSupplier = Objects.requireNonNull(nowMillisSupplier, "nowMillisSupplier");
    }

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String zoneId = readOptionalString(payload, "zone");
        Double spiritQi = readOptionalDouble(payload, "spirit_qi");
        Integer dangerLevel = readOptionalInteger(payload, "danger_level");
        if (zoneId == null || zoneId.isBlank() || spiritQi == null || dangerLevel == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring zone_info payload because required fields 'zone', 'spirit_qi', or 'danger_level' are missing or invalid"
            );
        }

        ZoneState zoneState = ZoneState.create(
            zoneId,
            readOptionalString(payload, "display_name"),
            spiritQi,
            dangerLevel,
            nowMillisSupplier.getAsLong()
        );
        if (zoneState.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring zone_info payload because the normalized zone state was empty"
            );
        }

        return ServerDataDispatch.handledWithZoneState(
            envelope.type(),
            zoneState,
            "Routed zone_info payload for zone '" + zoneState.zoneId() + "'"
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

    private static Integer readOptionalInteger(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        return primitive.getAsInt();
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }
}
