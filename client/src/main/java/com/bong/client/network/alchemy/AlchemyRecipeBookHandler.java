package com.bong.client.network.alchemy;

import com.bong.client.alchemy.state.RecipeScrollStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

/** plan-alchemy-v1 §4 — `alchemy_recipe_book` payload → {@link RecipeScrollStore}. */
public final class AlchemyRecipeBookHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            int currentIndex = p.has("current_index") && p.get("current_index").isJsonPrimitive()
                ? p.get("current_index").getAsInt() : 0;
            List<RecipeScrollStore.RecipeEntry> entries = new ArrayList<>();
            JsonArray arr = p.has("learned") && p.get("learned").isJsonArray()
                ? p.getAsJsonArray("learned") : null;
            if (arr != null) {
                for (JsonElement el : arr) {
                    if (!el.isJsonObject()) continue;
                    JsonObject e = el.getAsJsonObject();
                    entries.add(new RecipeScrollStore.RecipeEntry(
                        readString(e, "id", ""),
                        readString(e, "display_name", ""),
                        readString(e, "body_text", ""),
                        readString(e, "author", "散修"),
                        readString(e, "era", "末法"),
                        e.has("max_known") ? e.get("max_known").getAsInt() : 8
                    ));
                }
            }
            RecipeScrollStore.replace(new RecipeScrollStore.Snapshot(List.copyOf(entries), currentIndex));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied alchemy_recipe_book snapshot (" + entries.size() + " learned)");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "alchemy_recipe_book payload malformed: " + e.getMessage());
        }
    }

    private static String readString(JsonObject obj, String key, String fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() ? el.getAsString() : fallback;
    }
}
