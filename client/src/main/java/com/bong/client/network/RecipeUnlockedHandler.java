package com.bong.client.network;

import com.bong.client.craft.CraftStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/**
 * plan-craft-v1 P2 — `recipe_unlocked` 处理器。
 * 三渠道（残卷 / 师承 / 顿悟）触发，更新 {@link CraftStore} 解锁状态并通知 UI。
 *
 * <p>wire 中 `source` 为 tagged union（`kind` 字段）：</p>
 * <ul>
 *   <li>{@code {"kind":"scroll","item_template":"..."}}</li>
 *   <li>{@code {"kind":"mentor","npc_archetype":"..."}}</li>
 *   <li>{@code {"kind":"insight","trigger":"breakthrough" | "near_death" | "defeat_stronger"}}</li>
 * </ul>
 */
public final class RecipeUnlockedHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String recipeId = readString(payload, "recipe_id");
        if (recipeId == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring recipe_unlocked: missing recipe_id");
        }
        long unlockedAtTick = readLong(payload, "unlocked_at_tick");
        JsonElement sourceEl = payload.get("source");
        if (sourceEl == null || !sourceEl.isJsonObject()) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring recipe_unlocked: missing source");
        }
        JsonObject sourceObj = sourceEl.getAsJsonObject();
        String kind = readString(sourceObj, "kind");
        if (kind == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring recipe_unlocked: missing source.kind");
        }
        CraftStore.RecipeUnlockedEvent.UnlockSource source = switch (kind) {
            case "scroll" -> new CraftStore.RecipeUnlockedEvent.Scroll(
                readStringOrEmpty(sourceObj, "item_template"));
            case "mentor" -> new CraftStore.RecipeUnlockedEvent.Mentor(
                readStringOrEmpty(sourceObj, "npc_archetype"));
            case "insight" -> new CraftStore.RecipeUnlockedEvent.Insight(
                readStringOrEmpty(sourceObj, "trigger"));
            default -> null;
        };
        if (source == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring recipe_unlocked: unknown source.kind=" + kind);
        }
        CraftStore.recordUnlock(new CraftStore.RecipeUnlockedEvent(recipeId, source, unlockedAtTick));
        return ServerDataDispatch.handled(envelope.type(),
            "Applied recipe_unlocked " + recipeId + " (" + kind + ")");
    }

    private static String readString(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString())
            ? el.getAsString() : null;
    }

    private static String readStringOrEmpty(JsonObject obj, String name) {
        String s = readString(obj, name);
        return s == null ? "" : s;
    }

    private static long readLong(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0L;
        return el.getAsLong();
    }
}
