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
 * <p>未知 {@code gate.kind}（{@link RaceGate#fromWire} 返回 {@code null}）时**存入
 * {@link RaceGate#blocked()} 哨兵而非跳过该条**——fail-closed：该条目在
 * {@link RaceGateMetaStore} 里必须命中一个恒拒绝的 gate，不能因为跳过而退化成
 * "查不到 = any = 恒放行"（P3 opus verify LOW 项：跳过等价 fail-open，未知 kind 的
 * 装备/功法会被误判为无门槛放行）。这只应在 server 发出未来新 kind 而 client 未升级
 * 时发生，client 升级前一律置灰，属向前兼容的保守选择。缺 {@code id} 的条目仍旧跳过
 * （无 id 无法建立映射，跳过 vs 阻断皆无意义）。整个 payload 缺两数组字段时按空表处理
 * （{@code includingDefaultValueFields} 下 proto 恒发空数组，缺失只可能来自畸形 JSON）。
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

    /**
     * 解析一张 entry 数组为 id→RaceGate map。跳过畸形条目 / 缺 {@code id} / 缺
     * {@code gate} 对象（无法建立映射）。未知 {@code gate.kind}
     * ({@link RaceGate#fromWire} 返回 {@code null}) **不跳过**——存入
     * {@link RaceGate#blocked()} 哨兵，fail-closed（跳过会让该 id 在
     * {@link RaceGateMetaStore} 查表 miss，等价退化成 any 恒放行）。
     */
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
            // 未知 kind → fail-closed 哨兵（而非 continue 跳过）：该 id 必须在表里
            // 命中一个恒拒绝的 gate，不能查不到退化成 any 放行。
            out.put(id, gate == null ? RaceGate.blocked() : gate);
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
