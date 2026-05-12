package com.bong.client.network;

import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.regex.Pattern;

public final class MovementStateHandler implements ServerDataHandler {
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return handle(envelope, System.currentTimeMillis());
    }

    ServerDataDispatch handle(ServerDataEnvelope envelope, long nowMs) {
        if (!"movement_state".equals(envelope.type())) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring movement payload: unsupported type '" + envelope.type() + "'"
            );
        }

        MovementState parsed = parse(envelope.payload());
        if (parsed == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring movement_state payload: required fields missing or invalid"
            );
        }

        MovementStateStore.replace(parsed, nowMs);
        return ServerDataDispatch.handled(
            envelope.type(),
            "movement_state accepted (action=" + parsed.action().wireName()
                + " speed=" + parsed.currentSpeedMultiplier() + ")"
        );
    }

    private static MovementState parse(JsonObject payload) {
        Double speed = readDouble(payload, "current_speed_multiplier");
        Boolean staminaCostActive = readBoolean(payload, "stamina_cost_active");
        MovementState.Action action = MovementState.Action.fromWireName(readString(payload, "movement_action"));
        MovementState.ZoneKind zoneKind = MovementState.ZoneKind.fromWireName(readString(payload, "zone_kind"));
        Long dashCooldown = readLong(payload, "dash_cooldown_remaining_ticks");
        Double hitboxHeight = readDouble(payload, "hitbox_height_blocks");
        Double staminaCurrent = readDouble(payload, "stamina_current");
        Double staminaMax = readDouble(payload, "stamina_max");
        Boolean lowStamina = readBoolean(payload, "low_stamina");
        String rejectedAction = readOptionalMovementAction(payload, "rejected_action");

        if (speed == null
            || staminaCostActive == null
            || action == null
            || zoneKind == null
            || dashCooldown == null
            || hitboxHeight == null
            || staminaCurrent == null
            || staminaMax == null
            || lowStamina == null
            || speed < 0.0
            || dashCooldown < 0L
            || hitboxHeight < 0.0
            || staminaCurrent < 0.0
            || staminaMax <= 0.0
            || rejectedAction == null) {
            return null;
        }

        Long lastActionTick = readOptionalLong(payload, "last_action_tick");
        if (lastActionTick != null && lastActionTick < 0L) {
            return null;
        }

        return new MovementState(
            speed,
            staminaCostActive,
            action,
            zoneKind,
            dashCooldown,
            hitboxHeight,
            staminaCurrent,
            staminaMax,
            lowStamina,
            lastActionTick,
            rejectedAction,
            0L,
            0L,
            0L
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

    private static Double readDouble(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return null;
        double value = p.getAsDouble();
        return Double.isFinite(value) ? value : null;
    }

    private static Long readLong(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return null;
        String raw = p.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) {
            return null;
        }
        try {
            return p.getAsLong();
        } catch (NumberFormatException ex) {
            return null;
        }
    }

    private static Long readOptionalLong(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull()) return null;
        if (!el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return null;
        String raw = p.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) {
            return null;
        }
        try {
            return p.getAsLong();
        } catch (NumberFormatException ex) {
            return null;
        }
    }

    private static String readOptionalMovementAction(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull()) {
            return "";
        }
        if (!el.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isString()) {
            return null;
        }
        String value = p.getAsString();
        if (value.isEmpty()
            || "dash".equals(value)) {
            return value;
        }
        return null;
    }
}
