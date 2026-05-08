package com.bong.client.combat.handler;

import com.bong.client.combat.store.AscensionQuotaStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/** Handles {@code ascension_quota} snapshots. */
public final class AscensionQuotaHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        int occupiedSlots = readInt(payload, "occupied_slots", 0);
        int quotaLimit = readInt(payload, "quota_limit", 0);
        int availableSlots = readInt(
            payload,
            "available_slots",
            Math.max(0, quotaLimit - occupiedSlots)
        );
        double totalWorldQi = readDouble(payload, "total_world_qi", 0.0);
        double quotaK = readDouble(payload, "quota_k", 0.0);
        String quotaBasis = readString(payload, "quota_basis", "");
        AscensionQuotaStore.replace(new AscensionQuotaStore.State(
            occupiedSlots,
            quotaLimit,
            availableSlots,
            totalWorldQi,
            quotaK,
            quotaBasis
        ));
        return ServerDataDispatch.handled(envelope.type(), "ascension quota updated");
    }

    private static int readInt(JsonObject obj, String field, int fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double value = p.getAsDouble();
        if (!Double.isFinite(value)) return fallback;
        return Math.max(0, (int) value);
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double value = p.getAsDouble();
        return Double.isFinite(value) ? Math.max(0.0, value) : fallback;
    }

    private static String readString(JsonObject obj, String field, String fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : fallback;
    }
}
