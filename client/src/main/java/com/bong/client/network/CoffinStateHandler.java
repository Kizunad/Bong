package com.bong.client.network;

import com.bong.client.hud.CoffinStateStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * 解析 server {@code coffin_state} payload，写入 {@link CoffinStateStore}。
 *
 * <p>plan-coffin-tiers-v1 P3 新增：读取可选字段 {@code coffin_grade}（string，
 * "mundane"/"jade"/"stone"/"bronze"）；不在棺内时字段缺失 → grade = null，
 * {@link CoffinStateStore.State#grade()} 返回 null。</p>
 */
public final class CoffinStateHandler implements ServerDataHandler {

    /** 合法档级集合（用于过滤未知 grade，不认识的归 null 而非崩溃）。 */
    private static final java.util.Set<String> KNOWN_GRADES =
        java.util.Set.of("mundane", "jade", "stone", "bronze");

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        if (!"coffin_state".equals(envelope.type())) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring coffin payload: unsupported type '" + envelope.type() + "'"
            );
        }

        Boolean inCoffin = readBoolean(envelope.payload(), "in_coffin");
        Double multiplier = readDouble(envelope.payload(), "lifespan_rate_multiplier");
        if (inCoffin == null || multiplier == null || multiplier <= 0.0) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring coffin_state payload: required fields missing or invalid"
            );
        }

        // coffin_grade 是可选字段：在棺内时由 server 设置；不在棺内时字段缺失。
        // 未知 grade 值（前向兼容防御）→ null（降级为 mundane 渲染）。
        String grade = readGrade(envelope.payload(), "coffin_grade");

        CoffinStateStore.replace(new CoffinStateStore.State(inCoffin, multiplier, grade));
        return ServerDataDispatch.handled(
            envelope.type(),
            "coffin_state accepted (in_coffin=" + inCoffin
                + " multiplier=" + multiplier
                + " grade=" + grade + ")"
        );
    }

    private static String readGrade(JsonObject obj, String field) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        String raw = element.getAsString();
        return KNOWN_GRADES.contains(raw) ? raw : null;
    }

    private static Boolean readBoolean(JsonObject obj, String field) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isBoolean() ? primitive.getAsBoolean() : null;
    }

    private static Double readDouble(JsonObject obj, String field) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : null;
    }
}
