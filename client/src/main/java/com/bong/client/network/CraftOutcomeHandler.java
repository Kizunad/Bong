package com.bong.client.network;

import com.bong.client.craft.CraftStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/**
 * plan-craft-v1 P2 — `craft_outcome` 处理器（成功 / 失败二选一）。
 *
 * <p>wire 形式：`{"type":"craft_outcome","kind":"completed"|"failed", ...}`。</p>
 */
public final class CraftOutcomeHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String kind = readString(payload, "kind");
        if (kind == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring craft_outcome: missing kind discriminator");
        }
        String recipeId = readString(payload, "recipe_id");
        if (recipeId == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring craft_outcome: missing recipe_id");
        }
        switch (kind) {
            case "completed" -> {
                CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
                    recipeId,
                    readString(payload, "output_template"),
                    readInt(payload, "output_count"),
                    readLong(payload, "completed_at_tick")
                ));
                return ServerDataDispatch.handled(envelope.type(),
                    "Applied craft_outcome::completed " + recipeId);
            }
            case "failed" -> {
                CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.failed(
                    recipeId,
                    readString(payload, "reason"),
                    readInt(payload, "material_returned"),
                    readDouble(payload, "qi_refunded")
                ));
                return ServerDataDispatch.handled(envelope.type(),
                    "Applied craft_outcome::failed " + recipeId);
            }
            default -> {
                return ServerDataDispatch.noOp(envelope.type(),
                    "Ignoring craft_outcome with unknown kind=" + kind);
            }
        }
    }

    private static String readString(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString())
            ? el.getAsString() : null;
    }

    private static int readInt(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0;
        return el.getAsInt();
    }

    private static long readLong(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0L;
        return el.getAsLong();
    }

    private static double readDouble(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0.0;
        return el.getAsDouble();
    }
}
