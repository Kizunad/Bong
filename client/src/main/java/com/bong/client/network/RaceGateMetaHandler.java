package com.bong.client.network;

import com.bong.client.inventory.model.RaceGate;
import com.bong.client.inventory.state.RaceGateMetaStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * plan-race-system-v1 P3c — 解析 {@code race_gate_meta} payload，装入
 * {@link RaceGateMetaStore}。
 *
 * <p>两张 entry 数组（{@code item_wearer_race} / {@code technique_required_race}），
 * 每条 {@code {id, gate:{kind, species}}}。**只装非 any 条目**（server 侧只下发非 Any——
 * any 是默认）；本 handler 对显式 any 条目会解码但存入无害（{@code RaceGate.isAny} 恒放行）。
 *
 * <p>未知 {@code gate.kind}（{@link RaceGate#fromWire} 返回 {@code null}）时**跳过该条**
 * （fail-closed 于表：该条目查不到即回退 any 放行；但这只应在 server 发出未来新 kind
 * 而 client 未升级时发生，属向前兼容而非静默兜底 any 的正常路径）。缺 {@code id} 的
 * 条目同样跳过。整个 payload 缺两数组字段时按空表处理（{@code includingDefaultValueFields}
 * 下 proto 恒发空数组，缺失只可能来自畸形 JSON）。
 */
public final class RaceGateMetaHandler implements ServerDataHandler {

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Map<String, RaceGate> items = parseTable(readArray(payload, "item_wearer_race"));
        Map<String, RaceGate> techniques = parseTable(readArray(payload, "technique_required_race"));

        RaceGateMetaStore.replace(items, techniques);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied race_gate_meta: " + items.size() + " item gates, "
                + techniques.size() + " technique gates"
        );
    }

    /** 解析一张 entry 数组为 id→RaceGate map；跳过畸形 / 未知 kind / 缺 id 条目。 */
    private static Map<String, RaceGate> parseTable(JsonArray array) {
        Map<String, RaceGate> out = new LinkedHashMap<>();
        if (array == null) return out;
        for (JsonElement element : array) {
            if (element == null || !element.isJsonObject()) continue;
            JsonObject entry = element.getAsJsonObject();
            String id = readString(entry, "id");
            if (id == null || id.isBlank()) continue;
            JsonObject gateObj = readObject(entry, "gate");
            if (gateObj == null) continue;
            RaceGate gate = RaceGate.fromWire(readString(gateObj, "kind"), readSpecies(gateObj));
            if (gate == null) continue; // 未知 kind → 跳过（向前兼容，不塞进表）
            out.put(id, gate);
        }
        return out;
    }

    private static List<String> readSpecies(JsonObject gateObj) {
        JsonArray arr = readArray(gateObj, "species");
        List<String> out = new ArrayList<>();
        if (arr == null) return out;
        for (JsonElement el : arr) {
            if (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString()) {
                String s = el.getAsString();
                if (!s.isBlank()) out.add(s);
            }
        }
        return out;
    }

    private static JsonArray readArray(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonArray()) ? el.getAsJsonArray() : null;
    }

    private static JsonObject readObject(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonObject()) ? el.getAsJsonObject() : null;
    }

    private static String readString(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString())
            ? el.getAsString() : null;
    }
}
