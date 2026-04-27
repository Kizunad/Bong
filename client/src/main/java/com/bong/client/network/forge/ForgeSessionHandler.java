package com.bong.client.network.forge;

import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonObject;

/** plan-forge-v1 §4 — `forge_session` payload → {@link ForgeSessionStore}. */
public final class ForgeSessionHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            long sessionId = p.has("session_id") ? p.get("session_id").getAsLong() : 0;
            String bpId = p.has("blueprint_id") ? p.get("blueprint_id").getAsString() : "";
            String bpName = p.has("blueprint_name") ? p.get("blueprint_name").getAsString() : "";
            boolean active = !p.has("active") || p.get("active").getAsBoolean();
            String step = p.has("current_step") ? p.get("current_step").getAsString() : "done";
            int stepIdx = p.has("step_index") ? p.get("step_index").getAsInt() : 0;
            int tier = p.has("achieved_tier") ? p.get("achieved_tier").getAsInt() : 0;
            JsonObject stepState = p.has("step_state") ? p.getAsJsonObject("step_state") : new JsonObject();
            ForgeSessionStore.replace(new ForgeSessionStore.Snapshot(
                sessionId, bpId, bpName, active, step, stepIdx, tier, stepState.toString()));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied forge_session snapshot (session=" + sessionId + " step=" + step + ")");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "forge_session payload malformed: " + e.getMessage());
        }
    }
}
