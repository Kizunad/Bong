package com.bong.client.network.processing;

import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.bong.client.processing.state.FreshnessStore;
import com.bong.client.processing.state.ProcessingSessionStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/** plan-lingtian-process-v1 P3 — processing_session / freshness_update payload handler. */
public final class ProcessingServerDataHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return switch (envelope.type()) {
            case "processing_session" -> handleProcessingSession(envelope);
            case "freshness_update" -> handleFreshnessUpdate(envelope);
            default -> ServerDataDispatch.noOp(envelope.type(), "unsupported processing payload");
        };
    }

    private static ServerDataDispatch handleProcessingSession(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        boolean active = readBoolean(p, "active", true);
        if (!active) {
            ProcessingSessionStore.replace(ProcessingSessionStore.Snapshot.empty());
            return ServerDataDispatch.handled(envelope.type(), "Cleared processing session snapshot");
        }
        ProcessingSessionStore.replace(new ProcessingSessionStore.Snapshot(
            true,
            readString(p, "session_id", ""),
            ProcessingSessionStore.Kind.fromWire(readString(p, "kind", "drying")),
            readString(p, "recipe_id", ""),
            readInt(p, "progress_ticks", 0),
            readInt(p, "duration_ticks", 0),
            readString(p, "player_id", "")
        ));
        return ServerDataDispatch.handled(envelope.type(), "Applied processing session snapshot");
    }

    private static ServerDataDispatch handleFreshnessUpdate(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        FreshnessStore.upsert(
            readString(p, "item_uuid", ""),
            readFloat(p, "freshness", 1.0f),
            readString(p, "profile_name", "")
        );
        return ServerDataDispatch.handled(envelope.type(), "Applied freshness update");
    }

    private static boolean readBoolean(JsonObject obj, String key, boolean fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() && el.getAsJsonPrimitive().isBoolean() ? el.getAsBoolean() : fallback;
    }

    private static int readInt(JsonObject obj, String key, int fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() && el.getAsJsonPrimitive().isNumber() ? el.getAsInt() : fallback;
    }

    private static float readFloat(JsonObject obj, String key, float fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() && el.getAsJsonPrimitive().isNumber() ? el.getAsFloat() : fallback;
    }

    private static String readString(JsonObject obj, String key, String fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() ? el.getAsString() : fallback;
    }
}
