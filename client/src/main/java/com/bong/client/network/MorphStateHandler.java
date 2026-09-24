package com.bong.client.network;

import com.bong.client.inventory.model.MorphEntry;
import com.bong.client.inventory.state.MorphStateStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.HashMap;
import java.util.Map;

/**
 * plan-race-system-v1 PR-5b — 解析 {@code morph_state} payload，装入
 * {@link MorphStateStore}（server 侧 emit 见 {@code morph_state_emit.rs}）。
 *
 * <p>payload 形状（proto field 142，走通用 JsonFormat，无 enum 前缀，
 * {@code mode} 是 plain string）：
 * <pre>{@code
 * {"mode": "full" | "delta", "entries": [
 *   {"entity_id": 42, "model_kind": 0, "form_race_id": "whale",
 *    "form_body_plan_id": "whale", "active": true}
 * ]}
 * }</pre>
 *
 * <p>{@code mode="full"}：整表替换（只收 {@code active=true} 条目，全量语义下不应
 * 出现 {@code active=false}，出现也按跳过处理而非污染表）。
 * {@code mode="delta"}：逐条应用，{@code active=false} 从表中移除。
 * 未知 {@code mode}：不崩溃，跳过（fail-safe，日志由 dispatch 结果体现）。
 * 缺 {@code entity_id} 的条目跳过（无 key 无法建立映射）。
 */
public final class MorphStateHandler implements ServerDataHandler {

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String mode = readString(payload, "mode");
        JsonArray entries = readArray(payload, "entries");

        if ("full".equals(mode)) {
            Map<Integer, MorphEntry> active = new HashMap<>();
            if (entries != null) {
                for (JsonElement element : entries) {
                    JsonObject entry = asObject(element);
                    if (entry == null) continue;
                    Integer entityId = readInt(entry, "entity_id");
                    if (entityId == null) continue;
                    if (!readBoolean(entry, "active", true)) continue;
                    active.put(entityId, toMorphEntry(entry));
                }
            }
            MorphStateStore.applyFull(active);
            return ServerDataDispatch.handled(
                envelope.type(), "Applied morph_state full: " + active.size() + " morphed entities"
            );
        }

        if ("delta".equals(mode)) {
            int applied = 0;
            if (entries != null) {
                for (JsonElement element : entries) {
                    JsonObject entry = asObject(element);
                    if (entry == null) continue;
                    Integer entityId = readInt(entry, "entity_id");
                    if (entityId == null) continue;
                    boolean active = readBoolean(entry, "active", false);
                    MorphStateStore.applyDelta(entityId, active ? toMorphEntry(entry) : null);
                    applied++;
                }
            }
            return ServerDataDispatch.handled(
                envelope.type(), "Applied morph_state delta: " + applied + " entries"
            );
        }

        return ServerDataDispatch.handled(
            envelope.type(), "Ignored morph_state with unknown mode: " + mode
        );
    }

    private static MorphEntry toMorphEntry(JsonObject entry) {
        int modelKind = readInt(entry, "model_kind", 0);
        String formRaceId = readString(entry, "form_race_id");
        String formBodyPlanId = readString(entry, "form_body_plan_id");
        return new MorphEntry(
            modelKind,
            formRaceId == null ? "" : formRaceId,
            formBodyPlanId == null ? "" : formBodyPlanId
        );
    }

    private static JsonArray readArray(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonArray()) ? el.getAsJsonArray() : null;
    }

    private static JsonObject asObject(JsonElement element) {
        return (element != null && element.isJsonObject()) ? element.getAsJsonObject() : null;
    }

    private static String readString(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString())
            ? el.getAsString() : null;
    }

    private static Integer readInt(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isNumber())
            ? el.getAsInt() : null;
    }

    private static int readInt(JsonObject obj, String name, int fallback) {
        Integer value = readInt(obj, name);
        return value == null ? fallback : value;
    }

    private static boolean readBoolean(JsonObject obj, String name, boolean fallback) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isBoolean())
            ? el.getAsBoolean() : fallback;
    }
}
