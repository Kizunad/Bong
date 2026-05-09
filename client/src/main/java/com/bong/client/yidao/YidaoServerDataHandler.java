package com.bong.client.yidao;

import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

/** Handles yidao HUD and healer NPC AI server-data payloads. */
public final class YidaoServerDataHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        return switch (envelope.type()) {
            case "yidao_hud_state" -> handleHudState(envelope, payload);
            case "healer_npc_ai_state" -> handleNpcAiState(envelope, payload);
            default -> ServerDataDispatch.noOp(envelope.type(), "Unsupported yidao payload type");
        };
    }

    private ServerDataDispatch handleHudState(ServerDataEnvelope envelope, JsonObject payload) {
        YidaoHudStateStore.Snapshot next = new YidaoHudStateStore.Snapshot(
            readString(payload, "healer_id", ""),
            readSignedInt(payload, "reputation", 0),
            (float) readDouble(payload, "peace_mastery", 0d),
            readDouble(payload, "karma", 0d),
            readString(payload, "active_skill", ""),
            readStringList(payload, "patient_ids"),
            readNullableFloat(payload, "patient_hp_percent"),
            readNullableDouble(payload, "patient_contam_total"),
            readInt(payload, "severed_meridian_count", 0),
            readInt(payload, "contract_count", 0),
            readInt(payload, "mass_preview_count", 0)
        );
        YidaoHudStateStore.replace(next);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied yidao_hud_state (patients=" + next.patientIds().size() + ", karma=" + next.karma() + ")"
        );
    }

    private ServerDataDispatch handleNpcAiState(ServerDataEnvelope envelope, JsonObject payload) {
        YidaoNpcAiStateStore.Snapshot next = new YidaoNpcAiStateStore.Snapshot(
            readString(payload, "healer_id", ""),
            readString(payload, "active_action", ""),
            readInt(payload, "queue_len", 0),
            readSignedInt(payload, "reputation", 0),
            readBoolean(payload, "retreating", false)
        );
        YidaoNpcAiStateStore.upsert(next);
        if (next.clearSignal()) {
            return ServerDataDispatch.handled(
                envelope.type(),
                "Cleared healer_npc_ai_state (healer=" + next.healerId() + ")"
            );
        }
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied healer_npc_ai_state (action=" + next.activeAction() + ", queue=" + next.queueLen() + ")"
        );
    }

    private static List<String> readStringList(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonArray()) return List.of();
        JsonArray array = el.getAsJsonArray();
        List<String> out = new ArrayList<>(array.size());
        for (JsonElement item : array) {
            if (item != null && item.isJsonPrimitive()) {
                JsonPrimitive primitive = item.getAsJsonPrimitive();
                if (primitive.isString() && !primitive.getAsString().isBlank()) {
                    out.add(primitive.getAsString());
                }
            }
        }
        return out;
    }

    private static String readString(JsonObject obj, String field, String fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive primitive = el.getAsJsonPrimitive();
        return primitive.isString() ? primitive.getAsString() : fallback;
    }

    private static int readInt(JsonObject obj, String field, int fallback) {
        double value = readDouble(obj, field, fallback);
        return (int) Math.max(0, Math.min(Integer.MAX_VALUE, Math.round(value)));
    }

    private static int readSignedInt(JsonObject obj, String field, int fallback) {
        double value = readDouble(obj, field, fallback);
        if (value <= Integer.MIN_VALUE) return Integer.MIN_VALUE;
        if (value >= Integer.MAX_VALUE) return Integer.MAX_VALUE;
        return (int) Math.round(value);
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive primitive = el.getAsJsonPrimitive();
        if (!primitive.isNumber()) return fallback;
        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : fallback;
    }

    private static Float readNullableFloat(JsonObject obj, String field) {
        Double value = readNullableDouble(obj, field);
        return value == null ? null : value.floatValue();
    }

    private static Double readNullableDouble(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive primitive = el.getAsJsonPrimitive();
        if (!primitive.isNumber()) return null;
        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : null;
    }

    private static boolean readBoolean(JsonObject obj, String field, boolean fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive primitive = el.getAsJsonPrimitive();
        if (primitive.isBoolean()) return primitive.getAsBoolean();
        if (primitive.isNumber()) return primitive.getAsDouble() != 0d;
        return fallback;
    }
}
