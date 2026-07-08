package com.bong.client.network.forge;

import com.bong.client.forge.state.ForgeStationStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonObject;
import net.minecraft.util.math.BlockPos;

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
            // plan-forge-session-entry-wiring-v1 §4.1#3 —— proto ForgeStation.station_pos_x/y/z
            // 是非 optional 的 flat int32（Rust `pos: (i32,i32,i32)` 非 Option，与
            // alchemy_furnace 的 `Option<(i32,i32,i32)>` 不同）：PRINTER 的
            // includingDefaultValueFields() 保证三字段恒被下发。缺失只可能是旧/畸形 payload，
            // 此时保守降级为 null（与 AlchemyFurnaceHandler 同一降级语义），不连累其余字段。
            BlockPos pos = null;
            if (p.has("station_pos_x") && p.has("station_pos_y") && p.has("station_pos_z")
                    && p.get("station_pos_x").isJsonPrimitive() && p.get("station_pos_x").getAsJsonPrimitive().isNumber()
                    && p.get("station_pos_y").isJsonPrimitive() && p.get("station_pos_y").getAsJsonPrimitive().isNumber()
                    && p.get("station_pos_z").isJsonPrimitive() && p.get("station_pos_z").getAsJsonPrimitive().isNumber()) {
                pos = new BlockPos(
                    p.get("station_pos_x").getAsInt(),
                    p.get("station_pos_y").getAsInt(),
                    p.get("station_pos_z").getAsInt());
            }
            ForgeStationStore.replace(new ForgeStationStore.Snapshot(
                pos, stationId, tier, integrity, owner, hasSession));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied forge_station snapshot (tier=" + tier + ")");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "forge_station payload malformed: " + e.getMessage());
        }
    }
}
