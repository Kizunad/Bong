package com.bong.client.network;

import com.bong.client.hud.war.FactionWarHudStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-offscreen-war-v1 P9：处理 {@code faction_war_state} server_data payload。
 *
 * <p>解 JSON → 写 {@link FactionWarHudStore}。
 * Aftermath/无 active phase → {@link FactionWarHudStore#clear()}。
 * <br>守恒红线：payload <b>不含任何真元字段</b>（零真元）。
 * <br>reframe b：regionDescriptor 为匿名区域描述符，无具名宗门。
 */
public final class FactionWarHudHandler implements ServerDataHandler {

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String phase = readString(payload, "phase");

        // aftermath 或空 phase 表示战事已结束，清除 HUD
        if (phase.isEmpty() || "aftermath".equals(phase)) {
            FactionWarHudStore.clear();
            return ServerDataDispatch.handled(envelope.type(), "faction_war_state cleared");
        }

        String warId = readString(payload, "war_id");
        String zone = readString(payload, "zone");
        String regionDescriptor = readString(payload, "region_descriptor");
        List<Integer> groups = readIntArray(payload, "groups");
        int enlistCount = (int) readDouble(payload, "enlist_count", 0d);
        int mercenaryCount = (int) readDouble(payload, "mercenary_count", 0d);
        int interceptCount = (int) readDouble(payload, "intercept_count", 0d);
        int spectateCount = (int) readDouble(payload, "spectate_count", 0d);
        int winnerGroup = readOptionalInt(payload, "winner_group", -1);
        int loserGroup = readOptionalInt(payload, "loser_group", -1);

        FactionWarHudStore.replace(new FactionWarHudStore.State(
            warId, zone, regionDescriptor, phase, groups,
            enlistCount, mercenaryCount, interceptCount, spectateCount,
            winnerGroup, loserGroup
        ));
        return ServerDataDispatch.handled(envelope.type(), "faction_war_state updated phase=" + phase);
    }

    // ─────────── JSON helpers ─────────────────────────────────────────────────

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return "";
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : "";
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double v = p.getAsDouble();
        return Double.isFinite(v) ? v : fallback;
    }

    private static int readOptionalInt(JsonObject obj, String field, int fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull()) return fallback;
        if (!el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        return p.getAsInt();
    }

    private static List<Integer> readIntArray(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || !el.isJsonArray()) return List.of();
        JsonArray arr = el.getAsJsonArray();
        List<Integer> result = new ArrayList<>(arr.size());
        for (JsonElement item : arr) {
            if (item.isJsonPrimitive() && item.getAsJsonPrimitive().isNumber()) {
                result.add(item.getAsInt());
            }
        }
        return result;
    }
}
