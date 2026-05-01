package com.bong.client.network.alchemy;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import net.minecraft.util.math.BlockPos;

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
            boolean hasSession = p.has("has_session") && p.get("has_session").getAsBoolean();
            BlockPos pos = null;
            if (p.has("pos") && p.get("pos").isJsonArray()) {
                JsonArray arr = p.getAsJsonArray("pos");
                if (arr.size() == 3) {
                    pos = new BlockPos(arr.get(0).getAsInt(), arr.get(1).getAsInt(), arr.get(2).getAsInt());
                }
            }
            AlchemyFurnaceStore.replace(new AlchemyFurnaceStore.Snapshot(pos, tier, integrity, integrityMax, owner, hasSession));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied alchemy_furnace snapshot to AlchemyFurnaceStore (tier=" + tier + ")");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "alchemy_furnace payload malformed: " + e.getMessage());
        }
    }
}
