package com.bong.client.network;

import com.bong.client.cultivation.ColorKind;
import com.bong.client.cultivation.QiColorObservedState;
import com.bong.client.cultivation.QiColorObservedStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

public final class QiColorObservedHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        ColorKind main = ColorKind.fromWire(readString(payload, "main"));
        if (main == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring qi_color_observed payload: missing or unknown main color"
            );
        }
        QiColorObservedStore.replace(new QiColorObservedState(
            readString(payload, "observer"),
            readString(payload, "observed"),
            main,
            ColorKind.fromWire(readString(payload, "secondary")),
            readBoolean(payload, "is_chaotic"),
            readBoolean(payload, "is_hunyuan"),
            readDouble(payload, "realm_diff")
        ));
        return ServerDataDispatch.handled(envelope.type(), "Applied qi_color_observed snapshot");
    }

    private static String readString(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString())
            ? el.getAsString() : null;
    }

    private static boolean readBoolean(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isBoolean()
            && el.getAsBoolean();
    }

    private static double readDouble(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0.0;
        double v = el.getAsDouble();
        return Double.isFinite(v) ? v : 0.0;
    }
}
