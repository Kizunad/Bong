package com.bong.client.network.alchemy;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonObject;

/** plan-alchemy-v1 §4 — `alchemy_furnace` payload → {@link AlchemyFurnaceStore}. */
public final class AlchemyFurnaceHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            int tier = p.has("tier") ? p.get("tier").getAsInt() : 1;
            float integrity = (float) (p.has("integrity") ? p.get("integrity").getAsDouble() : 0.0);
            float integrityMax = (float) (p.has("integrity_max")
                ? p.get("integrity_max").getAsDouble() : 100.0);
            String owner = p.has("owner_name") && p.get("owner_name").isJsonPrimitive()
                ? p.get("owner_name").getAsString() : "self";
            AlchemyFurnaceStore.replace(new AlchemyFurnaceStore.Snapshot(tier, integrity, integrityMax, owner));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied alchemy_furnace snapshot to AlchemyFurnaceStore (tier=" + tier + ")");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "alchemy_furnace payload malformed: " + e.getMessage());
        }
    }
}
