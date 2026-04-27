package com.bong.client.network.forge;

import com.bong.client.forge.state.ForgeStationStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonObject;

/** plan-forge-v1 §4 — `forge_station` payload → {@link ForgeStationStore}. */
public final class ForgeStationHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            String stationId = p.has("station_id") ? p.get("station_id").getAsString() : "";
            int tier = p.has("tier") ? p.get("tier").getAsInt() : 1;
            float integrity = (float) (p.has("integrity") ? p.get("integrity").getAsDouble() : 1.0);
            String owner = p.has("owner_name") && p.get("owner_name").isJsonPrimitive()
                ? p.get("owner_name").getAsString() : "";
            boolean hasSession = p.has("has_session") && p.get("has_session").getAsBoolean();
            ForgeStationStore.replace(new ForgeStationStore.Snapshot(
                stationId, tier, integrity, owner, hasSession));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied forge_station snapshot (tier=" + tier + ")");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "forge_station payload malformed: " + e.getMessage());
        }
    }
}
