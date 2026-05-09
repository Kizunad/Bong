package com.bong.client.network;

import com.bong.client.combat.store.FullPowerStateStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

public final class FullPowerStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return handle(envelope, System.currentTimeMillis());
    }

    ServerDataDispatch handle(ServerDataEnvelope envelope, long nowMs) {
        return switch (envelope.type()) {
            case "full_power_charging_state" -> handleCharging(envelope, nowMs);
            case "full_power_release" -> handleRelease(envelope, nowMs);
            case "full_power_exhausted_state" -> handleExhausted(envelope, nowMs);
            default -> ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring full_power payload: unsupported type '" + envelope.type() + "'"
            );
        };
    }

    private static ServerDataDispatch handleCharging(ServerDataEnvelope envelope, long nowMs) {
        JsonObject payload = envelope.payload();
        String caster = readString(payload, "caster_uuid");
        Boolean active = readBoolean(payload, "active");
        Double qiCommitted = readDouble(payload, "qi_committed");
        Double targetQi = readDouble(payload, "target_qi");
        Long startedTick = readLong(payload, "started_tick");
        if (caster == null || active == null || qiCommitted == null || targetQi == null || startedTick == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring full_power_charging_state payload: required fields missing or invalid"
            );
        }
        if (active) {
            FullPowerStateStore.updateCharging(new FullPowerStateStore.ChargingState(
                true, caster, qiCommitted, targetQi, startedTick, nowMs
            ));
        } else {
            FullPowerStateStore.clearCharging();
        }
        return ServerDataDispatch.handled(
            envelope.type(),
            "full_power_charging_state accepted (caster=" + caster + " active=" + active + ")"
        );
    }

    private static ServerDataDispatch handleRelease(ServerDataEnvelope envelope, long nowMs) {
        JsonObject payload = envelope.payload();
        String caster = readString(payload, "caster_uuid");
        String target = readString(payload, "target_uuid");
        Double qiReleased = readDouble(payload, "qi_released");
        Long tick = readLong(payload, "tick");
        if (caster == null || qiReleased == null || tick == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring full_power_release payload: required fields missing or invalid"
            );
        }
        FullPowerStateStore.recordRelease(new FullPowerStateStore.ReleaseEvent(
            caster, target, qiReleased, tick, nowMs
        ));
        FullPowerStateStore.clearCharging();
        return ServerDataDispatch.handled(
            envelope.type(),
            "full_power_release accepted (caster=" + caster + " qi=" + qiReleased + ")"
        );
    }

    private static ServerDataDispatch handleExhausted(ServerDataEnvelope envelope, long nowMs) {
        JsonObject payload = envelope.payload();
        String caster = readString(payload, "caster_uuid");
        Boolean active = readBoolean(payload, "active");
        Long startedTick = readLong(payload, "started_tick");
        Long recoveryAtTick = readLong(payload, "recovery_at_tick");
        if (caster == null || active == null || startedTick == null || recoveryAtTick == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring full_power_exhausted_state payload: required fields missing or invalid"
            );
        }
        if (active) {
            FullPowerStateStore.updateExhausted(new FullPowerStateStore.ExhaustedState(
                true, caster, startedTick, recoveryAtTick, nowMs
            ));
        } else {
            FullPowerStateStore.clearExhausted();
        }
        return ServerDataDispatch.handled(
            envelope.type(),
            "full_power_exhausted_state accepted (caster=" + caster + " active=" + active + ")"
        );
    }

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : null;
    }

    private static Boolean readBoolean(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isBoolean() ? p.getAsBoolean() : null;
    }

    private static Long readLong(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isNumber() ? p.getAsLong() : null;
    }

    private static Double readDouble(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isNumber() ? p.getAsDouble() : null;
    }
}
