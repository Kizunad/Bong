package com.bong.client.network.forge;

import com.bong.client.forge.state.ForgeOutcomeStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;

/** plan-forge-v1 §4 — `forge_outcome` payload → {@link ForgeOutcomeStore}. */
public final class ForgeOutcomeHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            long sessionId = p.has("session_id") ? p.get("session_id").getAsLong() : 0;
            String bpId = p.has("blueprint_id") ? p.get("blueprint_id").getAsString() : "";
            String bucket = p.has("bucket") ? p.get("bucket").getAsString() : "waste";
            String weapon = p.has("weapon_item") && !p.get("weapon_item").isJsonNull()
                ? p.get("weapon_item").getAsString() : null;
            float quality = (float) (p.has("quality") ? p.get("quality").getAsDouble() : 0.0);
            String color = p.has("color") && !p.get("color").isJsonNull()
                ? p.get("color").getAsString() : null;
            int tier = p.has("achieved_tier") ? p.get("achieved_tier").getAsInt() : 0;
            boolean flawed = p.has("flawed_path") && p.get("flawed_path").getAsBoolean();

            StringBuilder sideFx = new StringBuilder();
            if (p.has("side_effects") && p.get("side_effects").isJsonArray()) {
                JsonArray arr = p.getAsJsonArray("side_effects");
                for (int i = 0; i < arr.size(); i++) {
                    if (i > 0) sideFx.append(",");
                    sideFx.append(arr.get(i).getAsString());
                }
            }

            ForgeOutcomeStore.replace(new ForgeOutcomeStore.Snapshot(
                sessionId, bpId, bucket, weapon, quality, color,
                sideFx.toString(), tier, flawed));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied forge_outcome snapshot (session=" + sessionId + " bucket=" + bucket + ")");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "forge_outcome payload malformed: " + e.getMessage());
        }
    }
}
