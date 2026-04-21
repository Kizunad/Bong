package com.bong.client.network;

import com.bong.client.combat.DefenseWindowState;
import com.bong.client.combat.DefenseWindowStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-HUD-v1 §3.2 截脉弹反窗口 client handler。
 * 收到 server 推的 defense_window 后写入 {@link DefenseWindowStore}，
 * {@code JiemaiRingHudPlanner} 据此渲染屏幕中心红环。
 */
public final class DefenseWindowHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Long durationMs = readLong(payload, "duration_ms");
        Long startedAtMs = readLong(payload, "started_at_ms");
        Long expiresAtMs = readLong(payload, "expires_at_ms");
        if (durationMs == null || startedAtMs == null || expiresAtMs == null
            || durationMs < 0 || startedAtMs < 0 || expiresAtMs < startedAtMs) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring defense_window payload: required fields missing or invalid"
            );
        }

        DefenseWindowStore.replaceSnapshot(DefenseWindowState.active(
            durationMs.intValue(), startedAtMs, expiresAtMs
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied defense_window (duration_ms=" + durationMs
                + " expires_at_ms=" + expiresAtMs + ")"
        );
    }

    private static Long readLong(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) return null;
        long value = primitive.getAsLong();
        return value < 0 ? null : value;
    }
}
