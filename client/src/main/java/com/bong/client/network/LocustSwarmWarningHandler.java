package com.bong.client.network;

import com.bong.client.state.VisualEffectState;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;

import java.util.Objects;
import java.util.function.LongSupplier;

public final class LocustSwarmWarningHandler {
    static final int WARNING_COLOR = 0xAA2222;
    private static final String ROUTE_TYPE = "locust_swarm_warning";
    private static final long DEFAULT_DURATION_MILLIS = 6_500L;

    private final LongSupplier nowMillisSupplier;

    public LocustSwarmWarningHandler() {
        this(System::currentTimeMillis);
    }

    LocustSwarmWarningHandler(LongSupplier nowMillisSupplier) {
        this.nowMillisSupplier = Objects.requireNonNull(nowMillisSupplier, "nowMillisSupplier");
    }

    public ServerDataDispatch handle(String jsonPayload) {
        JsonObject payload;
        try {
            JsonElement root = JsonParser.parseString(jsonPayload);
            if (!root.isJsonObject()) {
                return ServerDataDispatch.noOp(ROUTE_TYPE, "Ignoring locust_swarm_warning payload: root is not an object");
            }
            payload = root.getAsJsonObject();
        } catch (RuntimeException error) {
            return ServerDataDispatch.noOp(ROUTE_TYPE, "Ignoring malformed locust_swarm_warning payload");
        }

        Integer version = readOptionalInt(payload, "v");
        if (version == null || version != 1 || !"locust_swarm_warning".equals(readOptionalString(payload, "type"))) {
            return ServerDataDispatch.noOp(ROUTE_TYPE, "Ignoring locust_swarm_warning payload: invalid version or type");
        }

        String zone = normalizeText(readOptionalString(payload, "zone"));
        String message = normalizeText(readOptionalString(payload, "message"));
        if (zone.isEmpty() || message.isEmpty()) {
            return ServerDataDispatch.noOp(ROUTE_TYPE, "Ignoring locust_swarm_warning payload: missing zone or message");
        }

        ServerDataDispatch.ToastSpec toast = new ServerDataDispatch.ToastSpec(
            "灵蝗潮逼近：" + message,
            WARNING_COLOR,
            DEFAULT_DURATION_MILLIS
        );
        VisualEffectState effect = VisualEffectState.create(
            "pressure_jitter",
            0.65,
            DEFAULT_DURATION_MILLIS,
            nowMillisSupplier.getAsLong()
        );

        return ServerDataDispatch.handledWithEventAlert(
            ROUTE_TYPE,
            toast,
            effect,
            "Routed locust_swarm_warning payload for zone '" + zone + "'"
        );
    }

    private static String normalizeText(String value) {
        return value == null ? "" : value.trim();
    }

    private static String readOptionalString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }

    private static Integer readOptionalInt(JsonObject object, String fieldName) {
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
