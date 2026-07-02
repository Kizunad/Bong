package com.bong.client.network.alchemy;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
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
            // proto AlchemyFurnace.pos_x/pos_y/pos_z 是 flat optional int32（拆自 Rust
            // Option<(i32,i32,i32)>），ProtoServerDataBridge 不做 flat→array 重塑；三字段全存在
            // 且均为数字才重建 BlockPos，任一缺失/非数字 → pos 仍降级为 null（其余快照字段照常应用）。
            BlockPos pos = null;
            if (p.has("pos_x") && p.has("pos_y") && p.has("pos_z")
                    && p.get("pos_x").isJsonPrimitive() && p.get("pos_x").getAsJsonPrimitive().isNumber()
                    && p.get("pos_y").isJsonPrimitive() && p.get("pos_y").getAsJsonPrimitive().isNumber()
                    && p.get("pos_z").isJsonPrimitive() && p.get("pos_z").getAsJsonPrimitive().isNumber()) {
                pos = new BlockPos(p.get("pos_x").getAsInt(), p.get("pos_y").getAsInt(), p.get("pos_z").getAsInt());
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
