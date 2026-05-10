package com.bong.client.network;

import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/**
 * plan-craft-v1 P2 — `craft_session_state` 处理器。
 * server 每 1 秒 / 状态切换时推一次进度。
 */
public final class CraftSessionStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        boolean active = readBoolean(payload, "active");
        String recipeId = active ? readString(payload, "recipe_id") : null;
        long elapsed = readLong(payload, "elapsed_ticks");
        long total = readLong(payload, "total_ticks");
        int completed = readInt(payload, "completed_count");
        int totalCount = readInt(payload, "total_count", active ? 1 : 0);
        String error = readString(payload, "error");
        CraftStore.replaceSession(new CraftSessionStateView(
            active, recipeId, elapsed, total, completed, totalCount, error));
        return ServerDataDispatch.handled(envelope.type(),
            active
                ? "Applied craft_session_state(active=" + recipeId + " " + elapsed + "/" + total
                    + " completed=" + completed + "/" + totalCount + ")"
                : "Applied craft_session_state(idle)");
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

    private static long readLong(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0L;
        return el.getAsLong();
    }

    private static int readInt(JsonObject obj, String name) {
        return readInt(obj, name, 0);
    }

    private static int readInt(JsonObject obj, String name, int fallback) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return fallback;
        return el.getAsInt();
    }
}
