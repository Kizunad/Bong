package com.bong.client.network;

import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.util.ArrayList;
import java.util.List;

public final class BotanyHarvestProgressHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String sessionId = readOptionalString(payload, "session_id");
        if (sessionId == null || sessionId.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring botany_harvest_progress payload: required field 'session_id' is missing or invalid"
            );
        }
        Double progress = readOptionalDouble(payload, "progress");

        HarvestSessionViewModel model = HarvestSessionViewModel.create(
            sessionId,
            readOptionalString(payload, "target_id"),
            readOptionalString(payload, "target_name"),
            readOptionalString(payload, "plant_kind"),
            BotanyHarvestMode.fromWireName(readOptionalString(payload, "mode")),
            progress == null ? 0.0 : progress,
            readOptionalBoolean(payload, "auto_selectable") != Boolean.FALSE,
            readOptionalBoolean(payload, "request_pending") == Boolean.TRUE,
            readOptionalBoolean(payload, "interrupted") == Boolean.TRUE,
            readOptionalBoolean(payload, "completed") == Boolean.TRUE,
            readOptionalString(payload, "detail"),
            readHazardHints(payload),
            readOptionalFlatTriple(payload, "target_pos"),
            System.currentTimeMillis()
        );

        HarvestSessionStore.replace(model);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied botany_harvest_progress session '" + model.sessionId() + "' to HarvestSessionStore"
        );
    }

    private static String readOptionalString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }

    private static Double readOptionalDouble(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        return primitive.getAsDouble();
    }

    private static Boolean readOptionalBoolean(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isBoolean()) {
            return null;
        }
        return primitive.getAsBoolean();
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }

    /**
     * proto→legacy-JSON 桥不做 flat→array 重塑（见 ProtoServerDataBridge 类注释）：
     * proto {@code BotanyHarvestProgress.target_pos_x/y/z} 是 {@code optional double}
     * （Rust {@code Option<[f64;3]>} 拆三字段），生产走 protobuf 时读数组形状（旧
     * {@code readOptionalDoubleTriple}）永远拿 null → 目标坐标静默丢失。
     * 这里改读 flat 三字段，保持原 optional 降级语义：
     * 三个字段都在 → 组成坐标；三个都不在 → 合法的「无目标」，返回 null（采集会话仍应用）；
     * 只有部分在（残缺）→ 视为异常，同样返回 null，不拼半个坐标。
     */
    private static double[] readOptionalFlatTriple(JsonObject object, String prefix) {
        Double x = readOptionalDouble(object, prefix + "_x");
        Double y = readOptionalDouble(object, prefix + "_y");
        Double z = readOptionalDouble(object, prefix + "_z");
        if (x == null || y == null || z == null) {
            return null;
        }
        return new double[] {x, y, z};
    }

    private static List<String> readHazardHints(JsonObject object) {
        List<String> hints = new ArrayList<>();
        String single = readOptionalString(object, "hazard_hint");
        if (single != null && !single.isBlank()) {
            hints.add(single);
        }
        JsonElement element = object.get("hazard_hints");
        if (element != null && element.isJsonArray()) {
            for (JsonElement item : element.getAsJsonArray()) {
                if (item != null && item.isJsonPrimitive() && item.getAsJsonPrimitive().isString()) {
                    String value = item.getAsString();
                    if (!value.isBlank()) {
                        hints.add(value);
                    }
                }
            }
        }
        return List.copyOf(hints);
    }
}
