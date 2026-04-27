package com.bong.client.network.forge;

import com.bong.client.forge.state.BlueprintScrollStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;

/** plan-forge-v1 §4 — `forge_blueprint_book` payload → {@link BlueprintScrollStore}. */
public final class ForgeBlueprintBookHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            List<BlueprintScrollStore.Entry> entries = new ArrayList<>();
            if (p.has("learned") && p.get("learned").isJsonArray()) {
                JsonArray arr = p.getAsJsonArray("learned");
                for (int i = 0; i < arr.size(); i++) {
                    JsonObject entry = arr.get(i).getAsJsonObject();
                    String id = entry.has("id") ? entry.get("id").getAsString() : "";
                    String name = entry.has("display_name") ? entry.get("display_name").getAsString() : "";
                    int tierCap = entry.has("tier_cap") ? entry.get("tier_cap").getAsInt() : 1;
                    int stepCount = entry.has("step_count") ? entry.get("step_count").getAsInt() : 0;
                    entries.add(new BlueprintScrollStore.Entry(id, name, tierCap, stepCount));
                }
            }
            int idx = p.has("current_index") ? p.get("current_index").getAsInt() : 0;
            BlueprintScrollStore.replace(entries, idx);
            return ServerDataDispatch.handled(envelope.type(),
                "Applied forge_blueprint_book snapshot (" + entries.size() + " learned, index=" + idx + ")");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "forge_blueprint_book payload malformed: " + e.getMessage());
        }
    }
}
